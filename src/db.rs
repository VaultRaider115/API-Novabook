use crate::{config::Config, error::AppError};
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use std::time::Duration;

pub type DbPool = MySqlPool;

#[derive(Clone)]
pub struct AppState {
    pub pool: DbPool,
    pub config: Config,
}

pub async fn create_pool(database_url: &str) -> Result<DbPool, AppError> {
    MySqlPoolOptions::new()
        .max_connections(10)
        .min_connections(2)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Duration::from_secs(30))
        .connect(database_url)
        .await
        .map_err(AppError::Database)
}

/// Oculta la contraseña de una URL de conexión para poder registrarla en logs:
/// "mysql://user:secreto@host:3306/Libreria" → "mysql://user:****@host:3306/Libreria".
pub fn url_sin_contrasena(url: &str) -> String {
    let Some(inicio) = url.find("://").map(|i| i + 3) else {
        return url.to_string();
    };
    // Las credenciales terminan en la última '@' antes de la ruta.
    let fin_autoridad = url[inicio..].find('/').map_or(url.len(), |i| inicio + i);
    let Some(arroba) = url[inicio..fin_autoridad].rfind('@').map(|i| inicio + i) else {
        return url.to_string();
    };
    match url[inicio..arroba].find(':') {
        Some(dos_puntos) => format!("{}****{}", &url[..inicio + dos_puntos + 1], &url[arroba..]),
        None => url.to_string(),
    }
}

/// Revisa al arrancar que la base tenga las columnas que la API espera y, si
/// falta alguna, indica qué script aplicar (no detiene el servidor).
pub async fn verificar_esquema(pool: &DbPool) {
    let pendientes = [(
        "SELECT observaciones FROM devolucion LIMIT 0",
        "devolucion.observaciones",
        "migracion_observaciones_devolucion.sql",
    )];
    for (consulta, columna, script) in pendientes {
        if let Err(e) = sqlx::query(consulta).execute(pool).await {
            eprintln!(
                "[ESQUEMA] Falta la columna {columna}: aplica {script} a la base de datos. ({e})"
            );
        }
    }
}

/// Registra una acción en la tabla Bitacora para auditoría
pub async fn log_bitacora(
    pool: &DbPool,
    id_usuarios: i64,
    accion: &str,
    detalle: &str,
) -> Result<(), AppError> {
    log_bitacora_tipo(pool, id_usuarios, 1, accion, detalle).await
}

pub async fn log_bitacora_tipo(
    pool: &DbPool,
    id_usuarios: i64,
    id_tipo_evento: i32,
    accion: &str,
    detalle: &str,
) -> Result<(), AppError> {
    // Recorte al tamaño de las columnas (accion VARCHAR(100), detalle VARCHAR(255)):
    // si no, MariaDB rechaza el INSERT y el evento se pierde sin aviso.
    sqlx::query(
        "INSERT INTO bitacora (accion, detalle, id_usuarios, id_tipo_evento) \
         VALUES (?, ?, ?, ?)",
    )
    .bind(recortar(accion, 100))
    .bind(recortar(detalle, 255))
    .bind(id_usuarios)
    .bind(id_tipo_evento)
    .execute(pool)
    .await
    .map_err(AppError::Database)?;

    Ok(())
}

/// Recorta a `max` caracteres (no bytes), terminando en "…" si hubo recorte.
pub fn recortar(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut r: String = s.chars().take(max.saturating_sub(1)).collect();
    r.push('…');
    r
}

/// Indica si ya existe un registro cuyas columnas coinciden con los valores dados,
/// sin distinguir mayúsculas ni espacios al inicio/fin (catálogos sin repetidos).
/// `tabla`, `id_col` y las columnas son nombres fijos del código, nunca datos del
/// usuario; los valores siempre van como parámetros enlazados.
pub async fn existe_duplicado(
    pool: &DbPool,
    tabla: &'static str,
    id_col: &'static str,
    columnas: &[(&'static str, &str)],
    excluir_id: Option<i64>,
) -> Result<bool, AppError> {
    let mut qb = sqlx::QueryBuilder::<sqlx::MySql>::new(format!("SELECT 1 FROM {tabla} WHERE 1=1"));
    for (col, valor) in columnas {
        qb.push(format!(" AND LOWER(TRIM({col})) = LOWER(TRIM("))
            .push_bind(valor.to_string())
            .push("))");
    }
    if let Some(id) = excluir_id {
        qb.push(format!(" AND {id_col} <> ")).push_bind(id);
    }
    qb.push(" LIMIT 1");
    let fila = qb.build().fetch_optional(pool).await.map_err(AppError::Database)?;
    Ok(fila.is_some())
}

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use sqlx::Row;

/// Extrae de forma segura un i64 de una columna de MySQL sin importar si el tipo en BD es DECIMAL, BIGINT, INT o VARCHAR
pub fn row_get_i64(row: &sqlx::mysql::MySqlRow, col: &str) -> i64 {
    if let Ok(v) = row.try_get::<i64, _>(col) {
        v
    } else if let Ok(d) = row.try_get::<Decimal, _>(col) {
        d.to_i64().unwrap_or_default()
    } else if let Ok(v) = row.try_get::<i32, _>(col) {
        v as i64
    } else if let Ok(s) = row.try_get::<String, _>(col) {
        s.parse::<i64>().unwrap_or_default()
    } else {
        0
    }
}

/// Extrae de forma segura un Option<i64> de una columna de MySQL
pub fn row_get_opt_i64(row: &sqlx::mysql::MySqlRow, col: &str) -> Option<i64> {
    if let Ok(Some(v)) = row.try_get::<Option<i64>, _>(col) {
        Some(v)
    } else if let Ok(Some(d)) = row.try_get::<Option<Decimal>, _>(col) {
        d.to_i64()
    } else if let Ok(Some(v)) = row.try_get::<Option<i32>, _>(col) {
        Some(v as i64)
    } else if let Ok(Some(s)) = row.try_get::<Option<String>, _>(col) {
        s.parse::<i64>().ok()
    } else {
        None
    }
}

/// Extrae de forma segura un i32 de una columna de MySQL (soporta DECIMAL, INT, BIGINT, VARCHAR)
pub fn row_get_i32(row: &sqlx::mysql::MySqlRow, col: &str) -> i32 {
    if let Ok(v) = row.try_get::<i32, _>(col) {
        v
    } else if let Ok(d) = row.try_get::<Decimal, _>(col) {
        d.to_i32().unwrap_or_default()
    } else if let Ok(v) = row.try_get::<i64, _>(col) {
        v as i32
    } else if let Ok(s) = row.try_get::<String, _>(col) {
        s.parse::<i32>().unwrap_or_default()
    } else {
        0
    }
}

/// Extrae de forma segura un Option<i32> de una columna de MySQL
#[allow(dead_code)]
pub fn row_get_opt_i32(row: &sqlx::mysql::MySqlRow, col: &str) -> Option<i32> {
    if let Ok(Some(v)) = row.try_get::<Option<i32>, _>(col) {
        Some(v)
    } else if let Ok(Some(d)) = row.try_get::<Option<Decimal>, _>(col) {
        d.to_i32()
    } else if let Ok(Some(v)) = row.try_get::<Option<i64>, _>(col) {
        Some(v as i32)
    } else if let Ok(Some(s)) = row.try_get::<Option<String>, _>(col) {
        s.parse::<i32>().ok()
    } else {
        None
    }
}

/// Extrae de forma segura un f64 de una columna de MySQL (soporta DECIMAL, DOUBLE, FLOAT, VARCHAR)
pub fn row_get_f64(row: &sqlx::mysql::MySqlRow, col: &str) -> f64 {
    if let Ok(v) = row.try_get::<f64, _>(col) {
        v
    } else if let Ok(d) = row.try_get::<Decimal, _>(col) {
        d.to_f64().unwrap_or_default()
    } else if let Ok(s) = row.try_get::<String, _>(col) {
        s.parse::<f64>().unwrap_or_default()
    } else if let Ok(v) = row.try_get::<i64, _>(col) {
        v as f64
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::{recortar, url_sin_contrasena};

    #[test]
    fn oculta_la_contrasena_de_la_url() {
        assert_eq!(
            url_sin_contrasena("mysql://root:s3cr3t@localhost:3306/Libreria"),
            "mysql://root:****@localhost:3306/Libreria"
        );
        // Contraseña con '@' y ':' dentro
        assert_eq!(
            url_sin_contrasena("mysql://app:p@ss:w@db:3306/Libreria"),
            "mysql://app:****@db:3306/Libreria"
        );
        // Sin contraseña o sin credenciales: se deja igual
        assert_eq!(url_sin_contrasena("mysql://app@db/Libreria"), "mysql://app@db/Libreria");
        assert_eq!(url_sin_contrasena("mysql://db:3306/Libreria"), "mysql://db:3306/Libreria");
    }

    #[test]
    fn recorta_por_caracteres_sin_romper_acentos() {
        assert_eq!(recortar("Devolución", 20), "Devolución");
        let largo = "ñ".repeat(300);
        let r = recortar(&largo, 255);
        assert_eq!(r.chars().count(), 255);
        assert!(r.ends_with('…'));
    }
}
