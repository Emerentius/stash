use std::{fs::Metadata, path::PathBuf};

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
    Push,
    Show { index: Option<u32> },
    Pop,
    Clear,
}

struct Data {
    path: PathBuf,
    metadata: Metadata,
}

impl Data {
    fn detect(proj_dir: &ProjectDirs) -> Result<Vec<Data>> {
        let mut stashes = proj_dir
            .data_dir()
            .fs_err_read_dir()?
            .map(|entry| {
                let entry = entry?;
                Ok(Data {
                    path: entry.path(),
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

    fn get_newest(proj_dir: &ProjectDirs) -> Result<Option<Data>> {
        Ok(Self::detect(proj_dir)?.into_iter().next())
    }

    fn get(proj_dir: &ProjectDirs, idx: usize) -> Result<Option<Data>> {
        Ok(Self::detect(proj_dir)?.into_iter().nth(idx))
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    let proj_dirs = directories::ProjectDirs::from("", "", "stash")
        .ok_or_else(|| eyre!("couldn't get project dirs"))?;
    fs_err::create_dir_all(proj_dirs.data_dir())?;

    match args.command.unwrap_or(Subcommand::Push) {
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
                println!(
                    "{}: {}",
                    stash.path.file_name().unwrap().to_str().unwrap(),
                    stash_time
                );
            }
        }
        Subcommand::Push => {
            let last_number = Data::get_newest(&proj_dirs)?;
            let next_number = last_number.map_or(0, |last| {
                last.path
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .parse::<u32>()
                    .unwrap()
                    + 1
            });
            let mut file =
                fs_err::File::create(proj_dirs.data_dir().join(next_number.to_string()))?;
            std::io::copy(&mut std::io::stdin().lock(), &mut file)?;
        }
        Subcommand::Show { index } => {
            let desired_stash = Data::get(&proj_dirs, index.unwrap_or(0) as usize)?;
            print_stash(desired_stash.as_ref())?;
        }
        Subcommand::Pop => {
            let desired_stash = Data::get_newest(&proj_dirs)?;
            print_stash(desired_stash.as_ref())?;
            if let Some(stash) = desired_stash {
                fs_err::remove_file(stash.path)?;
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

fn print_stash(stash: Option<&Data>) -> Result<()> {
    match stash {
        Some(stash) => {
            let mut file = fs_err::File::open(&stash.path)?;
            let stdout = std::io::stdout();
            std::io::copy(&mut file, &mut stdout.lock())?;
        }
        None => eprintln!("Stash does not exist"),
    }
    Ok(())
}
