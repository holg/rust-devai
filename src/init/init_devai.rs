use crate::init::embedded_files::{
	get_embedded_command_agent_files, get_embedded_new_command_agent_files, get_embedded_new_solo_agent_files,
	EmbeddedFile,
};
use crate::init::migrate_devai::migrate_devai_0_1_0_if_needed;
use crate::run::{DevaiDir, DirContext};
use crate::support::files::current_dir;
use crate::Result;
use simple_fs::{ensure_dir, list_files, SPath};
use std::collections::HashSet;
use std::fs::write;
use std::path::Path;

// -- Config Content
const DEVAI_CONFIG_FILE_CONTENT: &str = include_str!("../../_base/config.toml");

// -- Doc Content
const DEVAI_DOC_RHAI_CONTENT: &str = include_str!("../../_base/doc/rhai.md");

pub fn init_devai_files() -> Result<DirContext> {
	// TODO: Add more logic to tell the user what has been created, where ...
	if let Some(dir_context) = DirContext::load()? {
		create_or_refresh_devai_files(dir_context.devai_parent_dir())?;
		Ok(dir_context)
	} else {
		create_or_refresh_devai_files(&current_dir()?)?;
		let dir_context = DirContext::load()?.ok_or("Could not create the devai dir")?;
		Ok(dir_context)
	}
}

/// Create or refresh missing file a devai dir
fn create_or_refresh_devai_files(devai_parent_dir: &SPath) -> Result<()> {
	let devai_dir = &DevaiDir::from_parent_dir(devai_parent_dir)?;

	ensure_dir(devai_dir)?;

	// -- Create the default agent files
	let devai_agent_default_dir = devai_dir.get_command_agent_default_dir()?;
	ensure_dir(devai_agent_default_dir)?;
	ensure_dir(devai_dir.get_command_agent_custom_dir()?)?;
	for dir in devai_dir.get_new_template_command_dirs()? {
		ensure_dir(dir)?;
	}
	for dir in devai_dir.get_new_template_solo_dirs()? {
		ensure_dir(dir)?;
	}

	// -- migrate_devai_0_1_0_if_needed
	migrate_devai_0_1_0_if_needed(devai_dir)?;

	// -- Create the default command agents if not present
	update_devai_files(
		devai_dir.get_command_agent_default_dir()?,
		get_embedded_command_agent_files(),
	)?;

	// -- Create the config file
	let config_path = devai_dir.get_config_toml_path()?;
	if !config_path.exists() {
		write(config_path, DEVAI_CONFIG_FILE_CONTENT)?;
	}

	// -- Create the new-template command default
	update_devai_files(
		devai_dir.get_new_template_command_default_dir()?,
		get_embedded_new_command_agent_files(),
	)?;

	// -- Create the new-template solo default
	update_devai_files(
		devai_dir.get_new_template_solo_default_dir()?,
		get_embedded_new_solo_agent_files(),
	)?;

	// -- Create the doc
	ensure_dir(devai_dir.get_doc_dir()?)?;
	let rhai_doc_path = devai_dir.get_doc_rhai_path()?;
	if !rhai_doc_path.exists() {
		write(rhai_doc_path, DEVAI_DOC_RHAI_CONTENT)?;
	}

	Ok(())
}

// region:    --- Support

fn update_devai_files(dir: impl AsRef<Path>, embedded_agent_file: &[&EmbeddedFile]) -> Result<()> {
	let dir = dir.as_ref();
	let existing_files = list_files(dir, Some(&["*.devai"]), None)?;
	let existing_names: HashSet<&str> = existing_files.iter().map(|f| f.file_name()).collect();

	for e_file in embedded_agent_file {
		if !existing_names.contains(e_file.name) {
			let path = Path::new(dir).join(e_file.name);
			write(&path, e_file.content)?;
		}
	}

	Ok(())
}

// endregion: --- Support
