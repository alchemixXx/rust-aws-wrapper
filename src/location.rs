use crate::custom_error::{CustomError, CustomResult};

pub fn get_current_dir() -> CustomResult<String> {
    let current_dir = std::env::current_dir()
        .map(|path| path.to_string_lossy().to_string())
        .map_err(|err| {
            CustomError::CommandExecution(format!("Failed to get current directory: {}", err))
        })?;

    println!("Current directory: {:?}", current_dir);
    // std::process::exit(1);

    Ok(current_dir)
}

pub fn get_repo_name() -> CustomResult<String> {
    let current_dir = get_current_dir()?;
    let repo_name = current_dir
        .split('/')
        .last()
        .ok_or_else(|| {
            CustomError::CommandExecution("Failed to extract repository name".to_string())
        })?
        .to_string();

    println!("Repository name: {:?}", repo_name);
    // std::process::exit(1);

    Ok(repo_name)
}
