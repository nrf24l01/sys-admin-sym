pub fn password() -> Result<String, String> {
    match std::env::var("GAME_SSH_PASSWORD") {
        Ok(password) => Ok(password),
        Err(std::env::VarError::NotPresent) => rpassword::prompt_password("Game password: ")
            .map_err(|error| {
                format!("could not read password: {error}; set GAME_SSH_PASSWORD for scripts")
            }),
        Err(_) => Err("GAME_SSH_PASSWORD must contain valid Unicode".into()),
    }
}
