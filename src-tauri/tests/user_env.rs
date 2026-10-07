use corral_lib::user_env::{registry_args, is_missing_delete};

#[test]
fn genera_argumentos_para_agregar_variable_de_entorno() {
    assert_eq!(
        registry_args("OLLAMA_IGPU_ENABLE", Some("1")),
        vec![
            "add",
            "HKCU\\Environment",
            "/v",
            "OLLAMA_IGPU_ENABLE",
            "/t",
            "REG_SZ",
            "/d",
            "1",
            "/f",
        ]
    );
}

#[test]
fn genera_argumentos_para_eliminar_variable_de_entorno() {
    assert_eq!(
        registry_args("HIP_VISIBLE_DEVICES", None),
        vec![
            "delete",
            "HKCU\\Environment",
            "/v",
            "HIP_VISIBLE_DEVICES",
            "/f",
        ]
    );
}

#[test]
fn conserva_espacios_y_comillas_en_un_solo_argumento() {
    assert_eq!(
        registry_args("TEST_VALUE", Some(r#"valor con "comillas""#)),
        vec![
            "add",
            "HKCU\\Environment",
            "/v",
            "TEST_VALUE",
            "/t",
            "REG_SZ",
            "/d",
            r#"valor con "comillas""#,
            "/f",
        ]
    );
}

#[test]
fn un_valor_vacio_agrega_la_variable() {
    assert_eq!(
        registry_args("EMPTY_VALUE", Some("")),
        vec![
            "add",
            "HKCU\\Environment",
            "/v",
            "EMPTY_VALUE",
            "/t",
            "REG_SZ",
            "/d",
            "",
            "/f",
        ]
    );
}

#[test]
fn ignora_solo_el_borrado_de_variables_ausentes_en_ingles_o_espanol() {
    for message in [
        "ERROR: The system was unable to find the specified registry key or value.",
        "ERROR: No se ha encontrado la clave o el valor del Registro especificado.",
        "ERROR: El sistema no puede encontrar la clave del Registro.",
        "ERROR: El sistema no ha podido encontrar la clave o el valor del Registro especificados.",
    ] {
        assert!(is_missing_delete(None, message));
        assert!(!is_missing_delete(Some("1"), message));
    }
    for message in ["ERROR: Access is denied.", "ERROR: Acceso denegado.", "error desconocido", ""] {
        assert!(!is_missing_delete(None, message));
    }
}
