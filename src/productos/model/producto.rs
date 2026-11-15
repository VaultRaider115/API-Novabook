use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoProducto {
    Libro,
    Revista,
}

impl TipoProducto {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Libro => "Libro",
            Self::Revista => "Revista",
        }
    }
}

/// Validador unificado de Código EAN-13
/// - Longitud: 13 dígitos
/// - Dígito verificador ponderado estándar EAN-13 (módulo 10)
/// - Clasificación por patrón:
///   * Libros: Prefijo 978 o 979 (ISBN-13)
///   * Revistas: Prefijo 977 (ISSN-13)
pub fn validar_patron_ean13(ean: i64) -> Result<TipoProducto, String> {
    if ean <= 0 {
        return Err("El código EAN debe ser un número positivo de 13 dígitos".to_string());
    }

    let ean_str = format!("{:013}", ean);
    if ean_str.len() != 13 || !ean_str.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!(
            "El código EAN '{}' no tiene exactamente 13 dígitos numéricos",
            ean_str
        ));
    }

    // Cálculo del dígito verificador EAN-13:
    // Posiciones impares (1, 3, 5, 7, 9, 11) peso 1
    // Posiciones pares (2, 4, 6, 8, 10, 12) peso 3
    let digits: Vec<u32> = ean_str.chars().map(|c| c.to_digit(10).unwrap()).collect();

    let sum_impar: u32 = digits[0] + digits[2] + digits[4] + digits[6] + digits[8] + digits[10];
    let sum_par: u32 = digits[1] + digits[3] + digits[5] + digits[7] + digits[9] + digits[11];
    let total = sum_impar + (sum_par * 3);
    let check_digit = (10 - (total % 10)) % 10;

    if check_digit != digits[12] {
        return Err(format!(
            "Dígito verificador inválido para EAN '{}': calculado {} pero se recibió {}",
            ean_str, check_digit, digits[12]
        ));
    }

    // Clasificación de patrón:
    if ean_str.starts_with("978") || ean_str.starts_with("979") {
        Ok(TipoProducto::Libro)
    } else if ean_str.starts_with("977") {
        Ok(TipoProducto::Revista)
    } else {
        Err(format!(
            "Patrón EAN '{}' no admitido: Solo se permiten libros con prefijo 978/979 (ISBN) o revistas con prefijo 977 (ISSN)",
            ean_str
        ))
    }
}

/// SKU interno: número de 6 a 7 dígitos (100000 – 9999999), según el
/// diagrama de procesos (paso 1, "Compra a la editorial").
pub const SKU_MIN: i64 = 100_000;
pub const SKU_MAX: i64 = 9_999_999;

pub fn validar_sku(sku: i64) -> Result<(), String> {
    if (SKU_MIN..=SKU_MAX).contains(&sku) {
        Ok(())
    } else {
        Err(format!("El SKU '{}' debe tener de 6 a 7 dígitos (sin ceros a la izquierda)", sku))
    }
}

#[cfg(test)]
mod tests {
    use super::{TipoProducto, validar_patron_ean13, validar_sku};

    #[test]
    fn sku_acepta_solo_6_a_7_digitos() {
        assert!(validar_sku(100_000).is_ok());
        assert!(validar_sku(884_102).is_ok());
        assert!(validar_sku(9_999_999).is_ok());
        assert!(validar_sku(99_999).is_err()); // 5 dígitos
        assert!(validar_sku(10_000_000).is_err()); // 8 dígitos
        assert!(validar_sku(0).is_err());
        assert!(validar_sku(-123_456).is_err());
    }

    #[test]
    fn ean_de_los_seeds_se_clasifican() {
        assert_eq!(validar_patron_ean13(9786070775390), Ok(TipoProducto::Libro));
        assert_eq!(validar_patron_ean13(9771234567010), Ok(TipoProducto::Revista));
        assert!(validar_patron_ean13(9786070775391).is_err()); // dígito verificador
        assert!(validar_patron_ean13(978607077539).is_err()); // 12 dígitos
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Libro {
    pub codigo_ean: i64,
    pub id_mueble: i32,
    pub id_proveedor: i32,
    pub id_ubicacion: i32,
    pub nombre_libro: String,
    pub precio: f64,
    pub cantidad: i32,
    pub sku: Option<i64>,
    #[sqlx(default)]
    #[serde(default)]
    pub generos: Vec<i32>,
    #[sqlx(default)]
    #[serde(default)]
    pub autores: Vec<i32>,
    #[sqlx(default)]
    #[serde(default)]
    pub generos_nombres: Option<String>,
    #[sqlx(default)]
    #[serde(default)]
    pub autores_nombres: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Revista {
    pub codigo_ean: i64,
    pub id_mueble: i32,
    pub id_proveedor: i32,
    pub id_editorial: i32,
    pub id_ubicacion: i32,
    pub nombre_revista: String,
    pub numero_edicion: Option<i32>,
    pub periodicidad: Option<String>,
    pub precio: f64,
    pub cantidad: i32,
    pub sku: Option<i64>,
    #[sqlx(default)]
    #[serde(default)]
    pub generos: Vec<i32>,
    #[sqlx(default)]
    #[serde(default)]
    pub generos_nombres: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RegistrarLibroRequest {
    pub codigo_ean: i64,
    pub id_mueble: i32,
    pub id_proveedor: i32,
    pub nombre_libro: String,
    pub precio: f64,
    #[serde(default)]
    pub cantidad: i32, // Opcional: el stock puede entrar después por Compras
    pub sku: Option<i64>,
    pub id_ubicacion: Option<i32>, // Default: 2 (Bodega)
    // Relación muchos a muchos con géneros
    #[serde(default)]
    pub generos: Vec<i32>,
    #[serde(default)]
    pub id_genero: Option<i32>, // Compatibilidad hacia atrás
    // Relación muchos a muchos con autores
    #[serde(default)]
    pub autores: Vec<i32>,
    #[serde(default)]
    pub autor: Option<String>, // Compatibilidad hacia atrás si se envía texto
}

#[derive(Debug, Clone, Deserialize)]
pub struct RegistrarRevistaRequest {
    pub codigo_ean: i64,
    pub id_mueble: i32,
    pub id_proveedor: i32,
    #[serde(default)]
    pub id_editorial: Option<i32>,
    pub nombre_revista: String,
    pub numero_edicion: Option<i32>,
    pub periodicidad: Option<String>,
    pub precio: f64,
    #[serde(default)]
    pub cantidad: i32, // Opcional: el stock puede entrar después por Compras
    pub sku: Option<i64>,
    pub id_ubicacion: Option<i32>, // Default: 2 (Bodega)
    // Relación muchos a muchos con géneros
    #[serde(default)]
    pub generos: Vec<i32>,
}
