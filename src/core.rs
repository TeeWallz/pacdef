use std::collections::HashSet;

use anyhow::{ensure, Context, Result};
use clap::ArgMatches;

use crate::action;
use crate::backend::{Backends, ToDoPerBackend};
use crate::cmd::run_edit_command;
use crate::ui::get_user_confirmation;
use crate::Group;

pub struct Pacdef {
    args: ArgMatches,
    groups: HashSet<Group>,
}

impl Pacdef {
    #[must_use]
    pub fn new(args: ArgMatches, groups: HashSet<Group>) -> Self {
        Self { args, groups }
    }

    #[allow(clippy::unit_arg)]
    pub fn run_action_from_arg(self) -> Result<()> {
        match self.args.subcommand() {
            Some((action::CLEAN, _)) => Ok(self.clean_packages()),
            Some((action::EDIT, groups)) => {
                self.edit_group_files(groups).context("editing group files")
            }
            Some((action::GROUPS, _)) => Ok(self.show_groups()),
            Some((action::SYNC, _)) => Ok(self.install_packages()),
            Some((action::UNMANAGED, _)) => Ok(self.show_unmanaged_packages()),
            Some((action::VERSION, _)) => Ok(self.show_version()),
            Some((_, _)) => todo!(),
            None => unreachable!(),
        }
    }

    fn get_missing_packages(&self) -> ToDoPerBackend {
        let mut to_install = ToDoPerBackend::new();

        for mut backend in Backends::iter() {
            backend.load(&self.groups);

            match backend.get_missing_packages_sorted() {
                Ok(diff) => to_install.push((backend, diff)),
                Err(e) => println!("WARNING: skipping backend '{}': {e}", backend.get_section()),
            };
        }

        to_install
    }

    fn install_packages(&self) {
        let to_install = self.get_missing_packages();

        if to_install.nothing_to_do_for_all_backends() {
            println!("nothing to do");
            return;
        }

        to_install.show();

        if !get_user_confirmation() {
            return;
        };

        to_install.install_missing_packages();
    }

    fn edit_group_files(&self, groups: &ArgMatches) -> Result<()> {
        let group_dir = crate::path::get_pacdef_group_dir()?;

        let files: Vec<_> = groups
            .get_many::<String>("group")
            .context("getting group from args")?
            .map(|file| {
                let mut buf = group_dir.clone();
                buf.push(file);
                buf
            })
            .collect();

        for file in &files {
            ensure!(
                file.exists(),
                "group file {} not found",
                file.to_string_lossy()
            );
        }

        let success = run_edit_command(&files)
            .context("running editor")?
            .success();

        ensure!(success, "editor exited with error");
        Ok(())
    }

    fn show_version(self) {
        println!("pacdef, version: {}", env!("CARGO_PKG_VERSION"));
    }

    fn show_unmanaged_packages(self) {
        let unmanaged_per_backend = &self.get_unmanaged_packages();

        for (backend, packages) in unmanaged_per_backend.iter() {
            if packages.is_empty() {
                continue;
            }
            println!("{}", backend.get_section());
            for package in packages {
                println!("  {package}");
            }
        }
    }

    fn get_unmanaged_packages(self) -> ToDoPerBackend {
        let mut result = ToDoPerBackend::new();

        for mut backend in Backends::iter() {
            backend.load(&self.groups);

            match backend.get_unmanaged_packages_sorted() {
                Ok(unmanaged) => result.push((backend, unmanaged)),
                Err(e) => println!("WARNING: skipping backend '{}': {e}", backend.get_section()),
            };
        }
        result
    }

    fn show_groups(self) {
        let mut vec: Vec<_> = self.groups.iter().collect();
        vec.sort_unstable();
        for g in vec {
            println!("{}", g.name);
        }
    }

    fn clean_packages(self) {
        let to_remove = self.get_unmanaged_packages();
        if to_remove.is_empty() {
            println!("nothing to do");
            return;
        }

        println!("Would remove the following packages and their dependencies:");
        for (backend, packages) in to_remove.iter() {
            if packages.is_empty() {
                continue;
            }

            println!("  {}", backend.get_section());
            for package in packages {
                println!("    {package}");
            }
        }

        if !get_user_confirmation() {
            return;
        };

        for (backend, packages) in to_remove.into_iter() {
            backend.remove_packages(packages);
        }
    }
}
