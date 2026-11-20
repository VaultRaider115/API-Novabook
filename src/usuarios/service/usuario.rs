use crate::{
    config::Config,
    db::{DbPool, log_bitacora, log_bitacora_tipo},
    error::AppError,
    middleware::generate_jwt,
    usuarios::{
        model::{
            ActualizarUsuarioRequest, CrearUsuarioRequest, LoginRequest, LoginResponse, RolEnum,
            UsuarioDto,
        },
        repo::UsuarioRepo,
    },
};

const MAX_PASSWORD_LENGTH: usize = crate::limites::PASSWORD_MAX;
const MAX_NAME_LENGTH: usize = crate::limites::USUARIO_NOMBRE_MAX;
const MAX_SURNAME_LENGTH: usize = crate::limites::USUARIO_APELLIDO_MAX;
const BCRYPT_COST: u32 = 10;

/// Hashea siempre la contraseña recibida: nunca se guarda tal cual,
/// aunque el texto parezca un hash bcrypt.
fn hash_password(plain: &str) -> Result<String, AppError> {
    bcrypt::hash(plain, BCRYPT_COST)
        .map_err(|e| AppError::Internal(format!("Error al hashear contraseña: {}", e)))
}

/// Solo acepta contraseñas almacenadas como hash bcrypt. Un valor en texto
/// plano en la BD (o cualquier hash inválido) nunca autentica.
fn verify_password(plain: &str, stored_hash: &str) -> bool {
    bcrypt::verify(plain, stored_hash).unwrap_or(false)
}

fn validate_password(password: &str, required: bool) -> Result<(), AppError> {
    if required && password.trim().is_empty() {
        return Err(AppError::BadRequest(
            "La contraseña no puede estar vacía".to_string(),
        ));
    }
    if password.chars().count() > MAX_PASSWORD_LENGTH {
        return Err(AppError::BadRequest(
            "La contraseña no puede exceder 16 caracteres".to_string(),
        ));
    }
    if !password.is_empty() && password.trim().is_empty() {
        return Err(AppError::BadRequest(
            "La contraseña no puede contener solo espacios".to_string(),
        ));
    }
    Ok(())
}

fn validate_required_text(value: &str, field: &str, max_length: usize) -> Result<(), AppError> {
    if value.trim().is_empty() {
        return Err(AppError::BadRequest(format!("{field} es obligatorio")));
    }
    if value.chars().count() > max_length {
        return Err(AppError::BadRequest(format!(
            "{field} no puede exceder {max_length} caracteres"
        )));
    }
    Ok(())
}

fn validate_optional_text(
    value: Option<&str>,
    field: &str,
    max_length: usize,
) -> Result<(), AppError> {
    if value.is_some_and(|text| text.chars().count() > max_length) {
        return Err(AppError::BadRequest(format!(
            "{field} no puede exceder {max_length} caracteres"
        )));
    }
    Ok(())
}

fn validate_phone(phone: Option<i64>) -> Result<(), AppError> {
    if phone.is_some_and(|value| !(1_000_000_000..=9_999_999_999).contains(&value)) {
        return Err(AppError::BadRequest(
            "El teléfono debe contener exactamente 10 dígitos".to_string(),
        ));
    }
    Ok(())
}

/// Teléfono recibido como texto o número: solo dígitos, exactamente 10 y sin
/// 0 inicial (como número, un 0 inicial se perdería sin avisar). Vacío o
/// ausente → sin teléfono. Errores 400 con mensaje claro.
fn parse_telefono(entrada: Option<&serde_json::Value>) -> Result<Option<i64>, AppError> {
    let texto = match entrada {
        None | Some(serde_json::Value::Null) => return Ok(None),
        Some(serde_json::Value::String(s)) => s.trim().to_string(),
        Some(serde_json::Value::Number(n)) => n.to_string(),
        Some(_) => {
            return Err(AppError::BadRequest(
                "El teléfono debe enviarse como texto o número".to_string(),
            ));
        }
    };
    if texto.is_empty() {
        return Ok(None);
    }
    if !texto.chars().all(|c| c.is_ascii_digit()) {
        return Err(AppError::BadRequest(
            "El teléfono solo puede contener dígitos (sin espacios, guiones ni letras)".to_string(),
        ));
    }
    if texto.starts_with('0') {
        return Err(AppError::BadRequest(
            "El teléfono no puede empezar con 0".to_string(),
        ));
    }
    if texto.len() != 10 {
        return Err(AppError::BadRequest(format!(
            "El teléfono debe tener exactamente 10 dígitos (tiene {})",
            texto.len()
        )));
    }
    texto
        .parse::<i64>()
        .map(Some)
        .map_err(|_| AppError::BadRequest("Teléfono inválido".to_string()))
}

/// Quita espacios al inicio/fin y colapsa los repetidos: "  Ana   María " → "Ana María".
fn limpiar_nombre(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn validate_person_name(value: &str, field: &str) -> Result<(), AppError> {
    if value.chars().any(|c| c.is_ascii_digit()) {
        return Err(AppError::BadRequest(format!(
            "{field} no puede contener números"
        )));
    }
    Ok(())
}

pub struct UsuarioService;

impl UsuarioService {
    /// Autentica al usuario verificando credenciales y emite un JWT
    pub async fn login(
        pool: &DbPool,
        config: &Config,
        req: LoginRequest,
    ) -> Result<LoginResponse, AppError> {
        if req.id_usuario <= 0 {
            return Err(AppError::BadRequest("ID de usuario inválido".to_string()));
        }
        validate_password(&req.contrasena, true)?;

        let usuario = UsuarioRepo::buscar_por_id(pool, req.id_usuario)
            .await?
            .ok_or_else(|| AppError::Unauthorized("Credenciales incorrectas".to_string()))?;

        // Solo bcrypt: ya no existe el respaldo de comparar en texto plano.
        let plano = req.contrasena.clone();
        let hash = usuario.contrasena.clone();
        let pass_valida = tokio::task::spawn_blocking(move || verify_password(&plano, &hash))
            .await
            .map_err(|e| AppError::Internal(format!("Error verificando contraseña: {}", e)))?;

        let activo = usuario.estado;

        if !pass_valida {
            return Err(AppError::Unauthorized(
                "Credenciales incorrectas".to_string(),
            ));
        }

        if !activo {
            return Err(AppError::Unauthorized(
                "Credenciales incorrectas".to_string(),
            ));
        }

        let rol_enum = RolEnum::from_id(usuario.id_roles).ok_or_else(|| {
            AppError::Internal(format!("Rol no válido en BD: {}", usuario.id_roles))
        })?;

        let token = generate_jwt(
            usuario.id_usuarios,
            rol_enum,
            &usuario.nombre,
            &config.jwt_secret,
            config.jwt_expiration_hours,
        )?;

        if let Err(error) = log_bitacora_tipo(
            pool,
            usuario.id_usuarios,
            2,
            "INICIO DE SESIÓN",
            "Inicio de sesión exitoso", // el rol ya se muestra en su propia columna
        )
        .await
        {
            eprintln!(
                "[AUDIT ERROR] No se pudo registrar inicio de sesión para usuario {}: {error}",
                usuario.id_usuarios
            );
        }

        let dto: UsuarioDto = usuario.into();

        Ok(LoginResponse {
            token,
            token_type: "Bearer".to_string(),
            expires_in_hours: config.jwt_expiration_hours,
            usuario: dto,
        })
    }

    pub async fn obtener_perfil(pool: &DbPool, id_usuario: i64) -> Result<UsuarioDto, AppError> {
        let usuario = UsuarioRepo::buscar_por_id(pool, id_usuario)
            .await?
            .ok_or_else(|| AppError::NotFound("Usuario no encontrado".to_string()))?;

        Ok(usuario.into())
    }

    pub async fn listar_usuarios(pool: &DbPool) -> Result<Vec<UsuarioDto>, AppError> {
        let usuarios = UsuarioRepo::listar_todos(pool).await?;
        Ok(usuarios.into_iter().map(Into::into).collect())
    }

    pub async fn obtener_usuario(pool: &DbPool, id: i64) -> Result<UsuarioDto, AppError> {
        let usuario = UsuarioRepo::buscar_por_id(pool, id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Usuario #{} no encontrado", id)))?;

        Ok(usuario.into())
    }

    pub async fn crear_usuario(
        pool: &DbPool,
        actor_id: i64,
        actor_rol: RolEnum,
        req: CrearUsuarioRequest,
    ) -> Result<UsuarioDto, AppError> {
        if actor_rol != RolEnum::Gerente {
            return Err(AppError::Forbidden(
                "Solo el Gerente puede dar de alta usuarios y asignar roles".to_string(),
            ));
        }
        if req.id_usuarios <= 0 {
            return Err(AppError::BadRequest(
                "ID de usuario debe ser mayor a 0".to_string(),
            ));
        }
        let mut req = req;
        req.telefono = parse_telefono(req.telefono_entrada.as_ref())?;
        validate_required_text(&req.nombre, "El nombre", MAX_NAME_LENGTH)?;
        validate_optional_text(
            req.apellido_paterno.as_deref(),
            "El apellido paterno",
            MAX_SURNAME_LENGTH,
        )?;
        validate_optional_text(
            req.apellido_materno.as_deref(),
            "El apellido materno",
            MAX_SURNAME_LENGTH,
        )?;

        validate_person_name(&req.nombre, "El nombre")?;
        if let Some(pat) = req.apellido_paterno.as_deref() {
            validate_person_name(pat, "El apellido paterno")?;
        }
        if let Some(mat) = req.apellido_materno.as_deref() {
            validate_person_name(mat, "El apellido materno")?;
        }

        validate_phone(req.telefono)?;
        if RolEnum::from_id(req.id_roles).is_none() {
            return Err(AppError::BadRequest(format!(
                "Rol inválido: {}",
                req.id_roles
            )));
        }
        validate_password(&req.contrasena, true)?;

        if UsuarioRepo::existe(pool, req.id_usuarios).await? {
            return Err(AppError::Conflict(format!(
                "El usuario con ID {} ya se encuentra registrado",
                req.id_usuarios
            )));
        }

        req.contrasena = hash_password(&req.contrasena)?;

        req.nombre = limpiar_nombre(&req.nombre);
        req.apellido_paterno = req
            .apellido_paterno
            .map(|s| limpiar_nombre(&s))
            .filter(|s| !s.is_empty());
        req.apellido_materno = req
            .apellido_materno
            .map(|s| limpiar_nombre(&s))
            .filter(|s| !s.is_empty());

        UsuarioRepo::crear(pool, &req).await?;

        let _ = log_bitacora(
            pool,
            actor_id,
            "CREAR USUARIO",
            &format!("Usuario creado: {} (ID: {})", req.nombre, req.id_usuarios),
        )
        .await;

        let usuario = UsuarioRepo::buscar_por_id(pool, req.id_usuarios)
            .await?
            .ok_or_else(|| {
                AppError::Internal("Error al recuperar usuario recién creado".to_string())
            })?;

        Ok(usuario.into())
    }

    pub async fn actualizar_usuario(
        pool: &DbPool,
        actor_id: i64,
        actor_rol: RolEnum,
        id: i64,
        req: ActualizarUsuarioRequest,
    ) -> Result<UsuarioDto, AppError> {
        let actual = UsuarioRepo::buscar_por_id(pool, id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Usuario #{} no encontrado", id)))?;
        let mut req = req;
        req.telefono = parse_telefono(req.telefono_entrada.as_ref())?;

        let gerente_id = RolEnum::Gerente as i32;
        if actor_rol != RolEnum::Gerente {
            // El Jefe puede editar datos de cuentas operativas, pero no roles ni cuentas de Gerente.
            if actual.id_roles == gerente_id {
                return Err(AppError::Forbidden(
                    "Solo el Gerente puede modificar una cuenta de Gerente".to_string(),
                ));
            }
            if req.id_roles.is_some_and(|r| r != actual.id_roles) {
                return Err(AppError::Forbidden(
                    "Solo el Gerente puede asignar o cambiar roles".to_string(),
                ));
            }
        }

        if id == actor_id && req.estatus == Some(false) {
            return Err(AppError::BadRequest(
                "No puedes desactivar tu propio usuario".to_string(),
            ));
        }
        if id == actor_id && req.id_roles.is_some_and(|r| r != actual.id_roles) {
            return Err(AppError::BadRequest(
                "No puedes cambiar tu propio rol".to_string(),
            ));
        }

        if let Some(r) = req.id_roles
            && RolEnum::from_id(r).is_none()
        {
            return Err(AppError::BadRequest(format!("Rol inválido: {}", r)));
        }

        // Nunca dejar el sistema sin un Gerente activo.
        let nuevo_rol = req.id_roles.unwrap_or(actual.id_roles);
        let nuevo_estado = req.estatus.unwrap_or(actual.estado);

        if actual.id_roles == gerente_id
            && actual.estado
            && (nuevo_rol != gerente_id || !nuevo_estado)
        {
            let gerentes_activos = UsuarioRepo::contar_activos_por_rol(pool, gerente_id).await?;
            if gerentes_activos <= 1 {
                // 409: la petición es válida pero choca con el estado actual.
                return Err(AppError::Conflict(
                    "No se puede: es el último Gerente activo y debe permanecer al menos uno \
                     (no se le puede cambiar el rol ni desactivar)"
                        .to_string(),
                ));
            }
        }

        if let Some(nombre) = req.nombre.as_deref() {
            validate_required_text(nombre, "El nombre", MAX_NAME_LENGTH)?;
            validate_person_name(nombre, "El nombre")?;
        }
        validate_optional_text(
            req.apellido_paterno.as_deref(),
            "El apellido paterno",
            MAX_SURNAME_LENGTH,
        )?;
        if let Some(pat) = req.apellido_paterno.as_deref() {
            validate_person_name(pat, "El apellido paterno")?;
        }
        validate_optional_text(
            req.apellido_materno.as_deref(),
            "El apellido materno",
            MAX_SURNAME_LENGTH,
        )?;
        if let Some(mat) = req.apellido_materno.as_deref() {
            validate_person_name(mat, "El apellido materno")?;
        }
        validate_phone(req.telefono)?;

        req.nombre = req.nombre.map(|s| limpiar_nombre(&s));
        req.apellido_paterno = req.apellido_paterno.map(|s| limpiar_nombre(&s));
        req.apellido_materno = req.apellido_materno.map(|s| limpiar_nombre(&s));

        if req.contrasena.as_deref() == Some("") {
            req.contrasena = None;
        }
        if let Some(pass) = req.contrasena.as_deref() {
            validate_password(pass, true)?;
        }
        if let Some(pass) = req.contrasena.as_deref() {
            req.contrasena = Some(hash_password(pass)?);
        }

        let actualizado = UsuarioRepo::actualizar(pool, id, &actual, &req).await?;

        let _ = log_bitacora(
            pool,
            actor_id,
            "ACTUALIZAR USUARIO",
            &format!("Usuario #{} actualizado", id),
        )
        .await;

        Ok(actualizado.into())
    }

    pub async fn eliminar_usuario(
        pool: &DbPool,
        actor_id: i64,
        actor_rol: RolEnum,
        id: i64,
    ) -> Result<(), AppError> {
        if id == actor_id {
            return Err(AppError::BadRequest(
                "No puedes eliminar tu propio usuario".to_string(),
            ));
        }
        let actual = UsuarioRepo::buscar_por_id(pool, id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Usuario #{} no encontrado", id)))?;

        let gerente_id = RolEnum::Gerente as i32;
        if actor_rol != RolEnum::Gerente && actual.id_roles == gerente_id {
            return Err(AppError::Forbidden(
                "Solo el Gerente puede dar de baja una cuenta de Gerente".to_string(),
            ));
        }

        if !actual.estado {
            return Err(AppError::Conflict(
                "El usuario ya se encuentra inactivo".to_string(),
            ));
        }

        if actual.id_roles == gerente_id {
            let gerentes_activos = UsuarioRepo::contar_activos_por_rol(pool, gerente_id).await?;
            if gerentes_activos <= 1 {
                return Err(AppError::Conflict(
                    "No puedes dar de baja al único Gerente activo del sistema".to_string(),
                ));
            }
        }

        let eliminado = UsuarioRepo::eliminar(pool, id).await?;
        if !eliminado {
            return Err(AppError::NotFound(format!("Usuario #{} no encontrado", id)));
        }

        let _ = log_bitacora(
            pool,
            actor_id,
            "ELIMINAR USUARIO",
            &format!("Usuario #{} eliminado", id),
        )
        .await;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        hash_password, limpiar_nombre, parse_telefono, validate_optional_text, validate_password,
        validate_phone, validate_required_text, verify_password,
    };
    use serde_json::json;

    #[test]
    fn telefono_acepta_texto_o_numero_de_10_digitos() {
        assert_eq!(parse_telefono(Some(&json!("8441112233"))).unwrap(), Some(8441112233));
        assert_eq!(parse_telefono(Some(&json!(8441112233_i64))).unwrap(), Some(8441112233));
        assert_eq!(parse_telefono(Some(&json!(" 8441112233 "))).unwrap(), Some(8441112233));
        assert_eq!(parse_telefono(None).unwrap(), None);
        assert_eq!(parse_telefono(Some(&json!(null))).unwrap(), None);
        assert_eq!(parse_telefono(Some(&json!(""))).unwrap(), None);
    }

    #[test]
    fn telefono_rechaza_cero_inicial_letras_y_largo() {
        let msg = |v| format!("{}", parse_telefono(Some(&v)).unwrap_err());
        assert!(msg(json!("0844111223")).contains("no puede empezar con 0"));
        assert!(msg(json!("844-111-22")).contains("solo puede contener dígitos"));
        assert!(msg(json!("84411122ab")).contains("solo puede contener dígitos"));
        assert!(msg(json!("844111223")).contains("exactamente 10"));
        assert!(msg(json!(844111223)).contains("exactamente 10"));
        assert!(msg(json!(true)).contains("texto o número"));
    }

    #[test]
    fn nombres_sin_espacios_sobrantes() {
        assert_eq!(limpiar_nombre("  Ana   María  "), "Ana María");
        assert_eq!(limpiar_nombre("Pérez\tLuévano"), "Pérez Luévano");
        assert_eq!(limpiar_nombre("   "), "");
    }

    #[test]
    fn plaintext_stored_password_never_authenticates() {
        // Antes había un respaldo que comparaba texto plano; ya no debe existir.
        assert!(!verify_password("admin123", "admin123"));
        assert!(!verify_password("", ""));
    }

    #[test]
    fn hash_looking_password_is_always_hashed() {
        let stored = hash_password("$2b$hola").unwrap();
        assert_ne!(stored, "$2b$hola");
        assert!(verify_password("$2b$hola", &stored));
    }

    #[test]
    fn seeded_test_accounts_still_authenticate() {
        // Hashes de libreria_unificada-definitivo.sql y contraseñas documentadas en CLAUDE.md.
        let seeds = [
            ("admin123", "$2b$10$GYRtA19hDIFbxL/ktJsMluzBL6RtxZu4KFSIOClBq14aG0Mvyh2zy"),
            ("2501", "$2b$10$JEis.kKv578RybJ7CPTKzOnzwJMf7iUbQWZwnpgSPmii0Rv8QLpyC"),
            ("8888", "$2b$10$9ZdsD93wmahmAOvWoEJ5SeI3yDWTZMNzlutEVfiiNbj4ZTgLPNUZy"),
            ("7777", "$2b$10$okBEvf28NTEBI3eA/jrO3euCYnsqKpk4w2T0eOdpNFucX4Y6pSUPq"),
        ];
        for (plain, hash) in seeds {
            assert!(verify_password(plain, hash), "falla la cuenta con contraseña {plain}");
            assert!(!verify_password("incorrecta", hash));
        }
    }

    #[test]
    fn password_accepts_maximum_length() {
        assert!(validate_password("abcdefghijklmnop", true).is_ok());
    }

    #[test]
    fn password_rejects_more_than_maximum_length() {
        assert!(validate_password("abcdefghijklmnopq", true).is_err());
    }

    #[test]
    fn required_password_rejects_blank_values() {
        assert!(validate_password("", true).is_err());
        assert!(validate_password("   ", true).is_err());
    }

    #[test]
    fn optional_password_accepts_empty_value() {
        assert!(validate_password("", false).is_ok());
    }

    #[test]
    fn names_enforce_required_and_schema_length_limits() {
        assert!(validate_required_text("Ana", "El nombre", 50).is_ok());
        assert!(validate_required_text(" ", "El nombre", 50).is_err());
        assert!(validate_required_text(&"a".repeat(51), "El nombre", 50).is_err());
        assert!(validate_optional_text(Some(&"a".repeat(30)), "Apellido", 30).is_ok());
        assert!(validate_optional_text(Some(&"a".repeat(31)), "Apellido", 30).is_err());
        assert!(validate_optional_text(None, "Apellido", 30).is_ok());
    }

    #[test]
    fn phone_must_have_exactly_ten_digits_when_provided() {
        assert!(validate_phone(None).is_ok());
        assert!(validate_phone(Some(8443509259)).is_ok());
        assert!(validate_phone(Some(844350925)).is_err());
        assert!(validate_phone(Some(10_000_000_000)).is_err());
    }
}
