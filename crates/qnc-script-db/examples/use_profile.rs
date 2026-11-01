//! Gives the active project of a QNC root its script profile again, as the profile of the same id
//! on this computer reads now (its names in this computer's language), through the module's own
//! writer: what "Use for project" in the profile editor does. The blocks stay; their kinds must be
//! in the profile. Usage: use_profile QNC_ROOT
fn main() -> Result<(), String> {
    let root = std::env::args().nth(1).ok_or("usage: use_profile QNC_ROOT")?;
    let reader = qnc_active_project_read::ActiveProjectReader::from_root(std::path::Path::new(&root)).map_err(|e| e.to_string())?;
    let snapshot = reader.read().map_err(|e| e.to_string())?;
    let target = qnc_db_broker::ProjectDbTarget::for_project(reader.settings_reader(), &snapshot.settings)?;
    let stored = qnc_script_db::read_profile(&target)?.ok_or("the project has no script profile")?;
    let (profiles, _) = qnc_script_profile::Profiles::load(std::path::Path::new(&root));
    let (profile, _) = profiles.list.into_iter().find(|(profile, _)| profile.profile_id == stored.profile_id).ok_or("this computer has no profile of that id")?;
    println!("project {} profile {}: {}", snapshot.settings.project_id, profile.profile_id, profile.name);
    qnc_script_db::set_profile(&target, profile)?;
    Ok(())
}
