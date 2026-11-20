//! Límites de largo de los campos de texto que captura el usuario.
//!
//! Son la referencia única: la API los valida aquí y el cliente Qt usa los
//! mismos valores en `src/core/Limites.h` (contador "12/60" y tope al escribir).
//! Si cambias uno, cambia también el otro.

use crate::error::AppError;

/// Nombre del cliente en el Punto de Venta (`ventas.nombre_cliente` admite 200).
pub const NOMBRE_CLIENTE_MAX: usize = 60;
/// Texto de búsqueda de productos (título, autor, EAN o SKU).
pub const BUSQUEDA_MAX: usize = 100;

/// Rechaza `valor` si pasa de `max` caracteres (no bytes: "ñ" cuenta como uno).
/// `campo` va al inicio del mensaje: "El nombre del cliente no puede pasar de…".
pub fn validar_largo(campo: &str, valor: &str, max: usize) -> Result<(), AppError> {
    let n = valor.chars().count();
    if n > max {
        return Err(AppError::BadRequest(format!(
            "{campo} no puede pasar de {max} caracteres (tiene {n})"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{NOMBRE_CLIENTE_MAX, validar_largo};

    #[test]
    fn acepta_hasta_el_limite() {
        assert!(validar_largo("El nombre", "", 5).is_ok());
        assert!(validar_largo("El nombre", "Ana", 5).is_ok());
        assert!(validar_largo("El nombre", "Laura", 5).is_ok());
    }

    #[test]
    fn rechaza_al_pasar_el_limite_con_mensaje_claro() {
        let err = validar_largo("El nombre del cliente", &"a".repeat(NOMBRE_CLIENTE_MAX + 1), NOMBRE_CLIENTE_MAX)
            .unwrap_err()
            .to_string();
        assert!(err.contains("El nombre del cliente no puede pasar de 60 caracteres (tiene 61)"), "{err}");
    }

    #[test]
    fn cuenta_caracteres_no_bytes() {
        // "ñ" ocupa 2 bytes en UTF-8 pero es un solo carácter.
        assert!(validar_largo("El nombre", "ñññññ", 5).is_ok());
        assert!(validar_largo("El nombre", "ññññññ", 5).is_err());
    }
}
