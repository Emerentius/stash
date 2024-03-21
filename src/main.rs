use std::{fs::Metadata, str::FromStr};

use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use clap::Parser;
use directories::ProjectDirs;
use eyre::{eyre, Result};
use fs_err::PathExt;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(subcommand)]
    command: Option<Subcommand>,
}

#[derive(clap::Subcommand, Debug)]
enum Subcommand {
    List,
    Push { name: Option<String> },
    Show { stash_id: Option<StashId> },
    Pop { name: Option<String> },
    Clear,
}

#[derive(Debug)]
struct StashId {
    name: String,
    index: Option<u32>,
}

impl FromStr for StashId {
    type Err = eyre::Report;

    fn from_str(id: &str) -> Result<Self, Self::Err> {
        let (name, index) = Data::parse_id_arg(id)?;
        Ok(StashId { name, index })
    }
}

struct Data {
    name: String,
    index: u32,
    metadata: Metadata,
}

impl Data {
    fn detect(proj_dir: &ProjectDirs) -> Result<Vec<Data>> {
        let mut stashes = proj_dir
            .data_dir()
            .fs_err_read_dir()?
            .map(|entry| {
                let entry = entry?;
                let path = PathBuf::from_path_buf(entry.path()).unwrap();
                let filename = path.file_name().unwrap();

                let (name, index) = Self::parse_id_internal(filename)?;

                Ok(Data {
                    name,
                    index,
                    metadata: entry.metadata()?,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        stashes.sort_by_key(|data| {
            std::cmp::Reverse(
                data.metadata
                    .created()
                    .expect("creation time not available"),
            )
        });
        Ok(stashes)
    }

    fn detect_named(proj_dir: &ProjectDirs, name: &str) -> Result<Vec<Data>> {
        let mut stashes = Self::detect(proj_dir)?;
        stashes.retain(|st| st.name == name);
        Ok(stashes)
    }

    fn parse_id_internal(stash_id: &str) -> Result<(String, u32)> {
        Self::_parse_id(stash_id, false).map(|(name, id)| (name, id.unwrap()))
    }

    fn parse_id_arg(stash_id: &str) -> Result<(String, Option<u32>)> {
        Self::_parse_id(stash_id, false)
    }

    fn _parse_id(stash_id: &str, internal: bool) -> Result<(String, Option<u32>)> {
        let separator = if internal { '_' } else { ':' };
        // TODO: need to filter out invalid characters
        match stash_id.rsplit_once(separator) {
            Some((name, index)) => {
                let index = index.parse().map_err(|_| {
                    eyre!("couldn't parse stash id. Expected <name>.<number>, got: {stash_id}")
                })?;
                Ok((name.to_owned(), Some(index)))
            }
            None => Ok((stash_id.to_owned(), None)),
        }
    }

    fn get_newest(proj_dir: &ProjectDirs, name: &str) -> Result<Option<Data>> {
        Self::get(proj_dir, name, 0)
    }

    fn get(proj_dir: &ProjectDirs, name: &str, idx: usize) -> Result<Option<Data>> {
        Ok(Self::detect_named(proj_dir, name)?.into_iter().nth(idx))
    }

    // this is also the internal stash_id
    fn filename(&self) -> String {
        format!("{}_{}", self.name, self.index)
    }

    fn path(&self, proj_dir: &ProjectDirs) -> PathBuf {
        Path::from_path(proj_dir.data_dir())
            .unwrap()
            .join(self.filename())
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    let proj_dirs = directories::ProjectDirs::from("", "", "stash")
        .ok_or_else(|| eyre!("couldn't get project dirs"))?;
    fs_err::create_dir_all(proj_dirs.data_dir())?;

    match args.command.unwrap_or(Subcommand::Push { name: None }) {
        Subcommand::List => {
            let stashes = Data::detect(&proj_dirs)?;
            for stash in stashes.into_iter().rev() {
                let stash_time = stash.metadata.created().unwrap();
                let unix_epoch = time::OffsetDateTime::UNIX_EPOCH;
                let stash_time =
                    unix_epoch + stash_time.duration_since(std::time::UNIX_EPOCH).unwrap();
                // TODO: use better time format
                let stash_time =
                    stash_time.format(&time::format_description::well_known::Rfc3339)?;
                println!("{}: {}", stash.filename(), stash_time);
            }
        }
        Subcommand::Push { name } => {
            let name = name.unwrap_or(String::new());
            let prev_stash = Data::get_newest(&proj_dirs, &name)?;
            let next_idx = prev_stash.map_or(0, |st| st.index + 1);
            let filename = format!("{name}.{next_idx}");
            let mut file = fs_err::File::create(proj_dirs.data_dir().join(filename))?;
            std::io::copy(&mut std::io::stdin().lock(), &mut file)?;
        }
        Subcommand::Show { stash_id } => {
            let (name, index) = match stash_id {
                Some(StashId { name, index }) => (Some(name), index),
                None => (None, None),
            };
            let desired_stash = Data::get(
                &proj_dirs,
                name.as_deref().unwrap_or_default(),
                index.unwrap_or(0) as usize,
            )?;
            print_stash(&proj_dirs, desired_stash.as_ref())?;
        }
        Subcommand::Pop { name } => {
            let name = name.as_deref().unwrap_or_default();
            let desired_stash = Data::get_newest(&proj_dirs, name)?;
            print_stash(&proj_dirs, desired_stash.as_ref())?;
            if let Some(stash) = desired_stash {
                fs_err::remove_file(stash.path(&proj_dirs))?;
            }
        }
        Subcommand::Clear => {
            for entry in proj_dirs.data_dir().fs_err_read_dir()? {
                fs_err::remove_file(entry?.path())?;
            }
        }
    }

    Ok(())
}

fn print_stash(proj_dir: &ProjectDirs, stash: Option<&Data>) -> Result<()> {
    match stash {
        Some(stash) => {
            let mut file = fs_err::File::open(stash.path(proj_dir))?;
            let stdout = std::io::stdout();
            std::io::copy(&mut file, &mut stdout.lock())?;
        }
        None => eprintln!("Stash does not exist"),
    }
    Ok(())
}
