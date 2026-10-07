pub fn registry_args(name: &str, value: Option<&str>) -> Vec<String> {
    match value {
        Some(value) => ["add", "HKCU\\Environment", "/v", name, "/t", "REG_SZ", "/d", value, "/f"].into_iter().map(str::to_string).collect(),
        None => ["delete", "HKCU\\Environment", "/v", name, "/f"].into_iter().map(str::to_string).collect(),
    }
}

pub fn is_missing_delete(value: Option<&str>, output: &str) -> bool {
    if value.is_some() {
        return false;
    }
    let output = output.to_lowercase();
    ["unable to find", "no se ha encontrado", "no puede encontrar", "no se pudo encontrar", "no se encuentra", "no ha podido encontrar"].into_iter().any(|message| output.contains(message))
}

#[cfg(windows)]
fn persist_variable(name: &str, value: Option<&str>) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let output = std::process::Command::new("reg")
        .args(registry_args(name, value))
        .creation_flags(0x0800_0000)
        .output().map_err(|e| format!("No se pudo guardar {name}: {e}"))?;
    let detail = format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    if output.status.success() || is_missing_delete(value, &detail) {
        return Ok(());
    }
    Err(format!("No se pudo guardar {name}: {}", detail.trim()))
}

pub fn persist(vars: &[(String, Option<String>)]) -> Result<(), String> {
    #[cfg(windows)]
    {
        for (name, value) in vars {
            persist_variable(name, value.as_deref())?;
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = vars;
        Err("La persistencia del perfil GPU solo está disponible en Windows".into())
    }
}
