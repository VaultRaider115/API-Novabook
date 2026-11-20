//! Límites de los datos que captura el usuario: largo de los textos y montos.
//!
//! Son la referencia única: la API los valida aquí y el cliente Qt usa los
//! mismos valores en `src/core/Limites.h` (contador "12/60" y tope al escribir).
//! Si cambias uno, cambia también el otro. Los largos no pasan del tamaño de
//! la columna en `docs/libreria_unificada-definitivo.sql`.

use crate::error::AppError;

// ---- Punto de venta y búsquedas --------------------------------------------
/// Nombre del cliente en el Punto de Venta (`ventas.nombre_cliente` admite 200).
pub const NOMBRE_CLIENTE_MAX: usize = 60;
/// Búsqueda de productos por título, autor, EAN o SKU (campos de EAN o
/// título y la consulta de inventario).
pub const BUSQUEDA_MAX: usize = 30;
/// Un EAN-13: si la búsqueda empieza con un prefijo de EAN (977, 978, 979)
/// y son solo dígitos, no puede pasar de 13.
pub const EAN_DIGITOS: usize = 13;
const PREFIJOS_EAN: [&str; 3] = ["977", "978", "979"];

/// Métodos de pago de una venta y de reembolso en una devolución de cliente.
/// "TRANSFERENCIA" se retiró: los registros viejos la conservan en la base
/// (`ventas.metodo_pago` es VARCHAR), pero ya no se acepta en uno nuevo.
pub const METODOS_PAGO: [&str; 2] = ["EFECTIVO", "TARJETA"];

// ---- Motivos (catálogo y notas) ---------------------------------------------
/// `motivo.nombre_motivo` VARCHAR(50).
pub const MOTIVO_NOMBRE_MAX: usize = 50;
/// Descripción del motivo, del mueble y observaciones (VARCHAR(200)).
pub const DESCRIPCION_MAX: usize = 200;
/// Observaciones / motivo al aprobar o rechazar (devolución, inventario,
/// devolución de cliente, recepción de la salida): VARCHAR(200).
pub const OBSERVACIONES_MAX: usize = 200;
/// Observaciones de una requisición (se guardan junto al folio en
/// `movimientos.observaciones`).
pub const OBSERVACIONES_REQUISICION_MAX: usize = 150;
/// Referencia de una solicitud de editorial (VARCHAR(150)).
pub const REFERENCIA_SOLICITUD_MAX: usize = 150;

// ---- Muebles ----------------------------------------------------------------
/// `mueble.codigo_mueble` VARCHAR(10) (P-… o B-…).
pub const MUEBLE_CODIGO_MAX: usize = 10;
/// `mueble.nombre_mueble` VARCHAR(50).
pub const MUEBLE_NOMBRE_MAX: usize = 50;

// ---- Catálogos ----------------------------------------------------------------
/// `genero.genero_literario` (la columna admite 50; se deja margen).
pub const GENERO_MAX: usize = 45;
/// `autores.nombre` VARCHAR(50).
pub const AUTOR_NOMBRE_MAX: usize = 50;
/// `autores.apellidos` VARCHAR(100).
pub const AUTOR_APELLIDOS_MAX: usize = 100;
/// `editorial.nombre_editorial` VARCHAR(100).
pub const EDITORIAL_NOMBRE_MAX: usize = 100;
/// RFC de proveedor y editorial (13 en persona física, 12 en moral).
pub const RFC_MAX: usize = 13;
/// `proveedor.nombre_proveedor` VARCHAR(50).
pub const PROVEEDOR_NOMBRE_MAX: usize = 50;
/// `proveedor.telefono` VARCHAR(15).
pub const TELEFONO_MAX: usize = 15;
/// `proveedor.correo` VARCHAR(100).
pub const CORREO_MAX: usize = 100;
/// `proveedor.direccion` VARCHAR(200) (calle, número, colonia y ciudad juntos).
pub const DIRECCION_MAX: usize = 200;
/// `proveedor.persona_contacto` (la columna admite 150).
pub const CONTACTO_MAX: usize = 100;

// ---- Productos y compras ------------------------------------------------------
/// Título de libro o revista (`libros.nombre_libro` VARCHAR(150)).
pub const TITULO_MAX: usize = 150;
/// `revistas.periodicidad` (la columna admite 100).
pub const PERIODICIDAD_MAX: usize = 30;
/// Número de factura con el prefijo "F-" (`compras_proveedor.factura` VARCHAR(30)).
pub const FACTURA_MAX: usize = 30;
/// Precio o costo más alto que se acepta (cuida errores de captura).
pub const MONTO_MAX: f64 = 99_999.99;

// ---- Usuarios -----------------------------------------------------------------
/// `usuarios.nombre` VARCHAR(50).
pub const USUARIO_NOMBRE_MAX: usize = 50;
/// `usuarios.apellido_paterno` / `apellido_materno` VARCHAR(30).
pub const USUARIO_APELLIDO_MAX: usize = 30;
/// Contraseña en claro antes del hash.
pub const PASSWORD_MAX: usize = 16;

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

/// Texto de búsqueda de productos: hasta [`BUSQUEDA_MAX`] caracteres y, si es
/// un EAN (solo dígitos que empiezan con 977/978/979), hasta 13 dígitos.
pub fn validar_busqueda_producto(q: &str) -> Result<(), AppError> {
    let q = q.trim();
    let es_ean = !q.is_empty() && q.chars().all(|c| c.is_ascii_digit()) && PREFIJOS_EAN.iter().any(|p| q.starts_with(p));
    if es_ean && q.len() > EAN_DIGITOS {
        return Err(AppError::BadRequest(format!(
            "Un EAN tiene {EAN_DIGITOS} dígitos (la búsqueda tiene {})",
            q.len()
        )));
    }
    validar_largo("La búsqueda", q, BUSQUEDA_MAX)
}

/// Igual que [`validar_largo`] para un campo opcional (`None` siempre pasa).
pub fn validar_largo_opt(campo: &str, valor: Option<&str>, max: usize) -> Result<(), AppError> {
    valor.map_or(Ok(()), |v| validar_largo(campo, v, max))
}

/// Precio o costo: debe ser un número mayor a $0 y no pasar de [`MONTO_MAX`].
/// Rechaza 0, negativos, NaN e infinito.
pub fn validar_monto(campo: &str, valor: f64) -> Result<(), AppError> {
    if !valor.is_finite() || valor <= 0.0 {
        return Err(AppError::BadRequest(format!("{campo} debe ser mayor a $0")));
    }
    if valor > MONTO_MAX {
        return Err(AppError::BadRequest(format!("{campo} no puede pasar de ${MONTO_MAX:.2}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        BUSQUEDA_MAX, MONTO_MAX, NOMBRE_CLIENTE_MAX, validar_busqueda_producto, validar_largo, validar_largo_opt,
        validar_monto,
    };

    #[test]
    fn busqueda_titulo_hasta_30_y_ean_hasta_13() {
        assert!(validar_busqueda_producto(&"a".repeat(BUSQUEDA_MAX)).is_ok());
        let err = validar_busqueda_producto(&"a".repeat(BUSQUEDA_MAX + 1)).unwrap_err().to_string();
        assert!(err.contains("no puede pasar de 30 caracteres"), "{err}");
        assert!(validar_busqueda_producto("9786073832014").is_ok());
        let err = validar_busqueda_producto("97860738320141").unwrap_err().to_string();
        assert!(err.contains("Un EAN tiene 13 dígitos"), "{err}");
        // Sin prefijo de EAN (un SKU, por ejemplo) solo aplica el límite general.
        assert!(validar_busqueda_producto("12345678901234").is_ok());
        assert!(validar_busqueda_producto("Cien años de soledad").is_ok());
    }

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

    #[test]
    fn opcional_vacio_pasa() {
        assert!(validar_largo_opt("La nota", None, 3).is_ok());
        assert!(validar_largo_opt("La nota", Some("abc"), 3).is_ok());
        assert!(validar_largo_opt("La nota", Some("abcd"), 3).is_err());
    }

    #[test]
    fn monto_debe_ser_mayor_a_cero() {
        for malo in [0.0, -1.0, -0.01, f64::NAN, f64::INFINITY, MONTO_MAX + 0.01] {
            assert!(validar_monto("El costo", malo).is_err(), "aceptó {malo}");
        }
        for bueno in [0.01, 150.0, MONTO_MAX] {
            assert!(validar_monto("El costo", bueno).is_ok(), "rechazó {bueno}");
        }
        let err = validar_monto("El costo unitario", 0.0).unwrap_err().to_string();
        assert!(err.contains("El costo unitario debe ser mayor a $0"), "{err}");
    }
}
