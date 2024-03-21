use std::fs::Metadata;

use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use clap::Parser;
use directories::ProjectDirs;
use eyre::{eyre, Result};
use fs_err::PathExt;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(subcommand)]
    command: Subcommand,
}

#[derive(clap::Subcommand, Debug)]
enum Subcommand {
    List,
    Store {
        name: String,
        #[clap(short, long)]
        append: bool,
    },
    Show {
        name: String,
        #[clap(short, long)]
        delete: bool,
    },
    Delete {
        name: String,
    },
    Clear,
}

struct Data {
    name: String,
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

                Ok(Data {
                    name: filename.to_owned(),
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

    fn get(proj_dir: &ProjectDirs, name: &str) -> Result<Option<Data>> {
        let data_dir = Path::from_path(proj_dir.data_dir())
            .ok_or(eyre!("non-utf8 data dir path"))?
            .to_owned();
        let file = data_dir.join(name);
        let metadata = match fs_err::metadata(file) {
            Ok(metadata) => metadata,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            err @ Err(_) => err?,
        };
        Ok(Some(Data {
            name: name.to_owned(),
            metadata,
        }))
    }

    // this is also the internal stash_id
    fn filename(&self) -> String {
        self.name.to_string()
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

    match args.command {
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
        Subcommand::Store { name, append } => {
            let filename = name.to_string();
            let path = proj_dirs.data_dir().join(filename);
            let mut file = fs_err::OpenOptions::new()
                .create(true)
                .write(true)
                .append(append)
                .open(path)?;
            std::io::copy(&mut std::io::stdin().lock(), &mut file)?;
        }
        Subcommand::Show { name, delete } => {
            let desired_stash = Data::get(&proj_dirs, &name)?;
            print_stash(&proj_dirs, desired_stash.as_ref())?;
            if delete {
                delete_stash(&proj_dirs, desired_stash)?
            }
        }
        Subcommand::Delete { name } => {
            let desired_stash = Data::get(&proj_dirs, &name)?;
            delete_stash(&proj_dirs, desired_stash)?;
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

fn delete_stash(proj_dirs: &ProjectDirs, stash: Option<Data>) -> Result<()> {
    if let Some(stash) = stash {
        fs_err::remove_file(stash.path(proj_dirs))?;
    }
    Ok(())
}
