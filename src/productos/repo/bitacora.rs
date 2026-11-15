use sqlx::Row;

use sqlx::{MySql, QueryBuilder};

use crate::{
    db::{DbPool, row_get_i64},
    error::AppError,
    productos::model::{BitacoraItem, BitacoraPagina, BitacoraQuery, MovimientoDiarioItem},
};

/// Categoría visible de un evento, derivada del tipo de evento y del texto de la acción.
const CATEGORIA_SQL: &str = "CASE \
    WHEN b.id_tipo_evento = 2 THEN 'ACCESO' \
    WHEN UPPER(b.accion) LIKE '%VENTA%' THEN 'VENTA' \
    WHEN UPPER(b.accion) LIKE '%COMPRA%' THEN 'COMPRA' \
    WHEN UPPER(b.accion) LIKE '%TRASLADO%' OR UPPER(b.accion) LIKE '%REQUISICI%' THEN 'TRASLADO' \
    WHEN UPPER(b.accion) LIKE '%DEVOLUC%' THEN 'DEVOLUCION' \
    WHEN UPPER(b.accion) LIKE '%USUARIO%' THEN 'USUARIOS' \
    WHEN UPPER(b.accion) LIKE 'REGISTRO %' OR UPPER(b.accion) LIKE '%LIBRO%' OR UPPER(b.accion) LIKE '%REVISTA%' OR UPPER(b.accion) LIKE '%PRODUCTO%' THEN 'INVENTARIO' \
    WHEN UPPER(b.accion) LIKE 'CREAR%' OR UPPER(b.accion) LIKE 'ACTUALIZAR%' OR UPPER(b.accion) LIKE 'ELIMINAR%' THEN 'CATALOGO' \
    ELSE 'SISTEMA' END";

fn fecha_valida(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

pub struct BitacoraRepo;

impl BitacoraRepo {
    pub async fn consultar_movimiento_diario(
        pool: &DbPool,
        fecha: Option<&str>,
    ) -> Result<Vec<MovimientoDiarioItem>, AppError> {
        let is_curdate = fecha.map(|f| f.trim().is_empty()).unwrap_or(true);

        let query_str = if is_curdate {
            r#"
            SELECT DATE_FORMAT(b.fecha_hora, '%Y-%m-%d %H:%i:%s') as fecha_hora,
                   CONCAT(u.nombre, ' ', COALESCE(u.apellido_paterno, '')) AS usuario,
                   COALESCE(r.nombre_rol, 'Sin Rol') AS rol,
                   b.accion,
                   b.detalle
            FROM bitacora b
            JOIN usuarios u ON b.id_usuarios = u.id_usuarios
            LEFT JOIN roles r ON u.id_roles = r.id_roles
            WHERE DATE(b.fecha_hora) = CURDATE()
            ORDER BY b.fecha_hora DESC
            "#
        } else {
            r#"
            SELECT DATE_FORMAT(b.fecha_hora, '%Y-%m-%d %H:%i:%s') as fecha_hora,
                   CONCAT(u.nombre, ' ', COALESCE(u.apellido_paterno, '')) AS usuario,
                   COALESCE(r.nombre_rol, 'Sin Rol') AS rol,
                   b.accion,
                   b.detalle
            FROM bitacora b
            JOIN usuarios u ON b.id_usuarios = u.id_usuarios
            LEFT JOIN roles r ON u.id_roles = r.id_roles
            WHERE DATE(b.fecha_hora) = ?
            ORDER BY b.fecha_hora DESC
            "#
        };

        let rows = if is_curdate {
            sqlx::query(query_str).fetch_all(pool).await
        } else {
            sqlx::query(query_str)
                .bind(fecha.unwrap())
                .fetch_all(pool)
                .await
        }
        .map_err(AppError::Database)?;

        let list = rows
            .into_iter()
            .map(|r| MovimientoDiarioItem {
                fecha_hora: r.get("fecha_hora"),
                usuario: r.get("usuario"),
                rol: r.get("rol"),
                accion: r.get("accion"),
                detalle: r.get("detalle"),
            })
            .collect();

        Ok(list)
    }

    /// Bitácora paginada por rango de fechas y categoría (más reciente primero).
    pub async fn consultar_pagina(
        pool: &DbPool,
        q: &BitacoraQuery,
    ) -> Result<BitacoraPagina, AppError> {
        let hoy: String = sqlx::query_scalar("SELECT DATE_FORMAT(CURDATE(), '%Y-%m-%d')")
            .fetch_one(pool)
            .await
            .map_err(AppError::Database)?;
        let desde = q.desde.clone().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| hoy.clone());
        let hasta = q.hasta.clone().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| desde.clone());
        if !fecha_valida(&desde) || !fecha_valida(&hasta) {
            return Err(AppError::BadRequest("Las fechas deben tener formato YYYY-MM-DD".to_string()));
        }
        if hasta < desde {
            return Err(AppError::BadRequest("La fecha final no puede ser anterior a la inicial".to_string()));
        }
        let categoria = q
            .categoria
            .as_deref()
            .map(|c| c.trim().to_uppercase())
            .filter(|c| !c.is_empty() && c != "TODOS");
        let per_page = q.per_page.unwrap_or(5).clamp(1, 100);
        let page = q.page.unwrap_or(0);

        let base = format!(
            "FROM (SELECT b.fecha_hora, b.accion, b.detalle, b.id_usuarios, {CATEGORIA_SQL} AS categoria \
             FROM bitacora b WHERE DATE(b.fecha_hora) BETWEEN "
        );

        let mut count: QueryBuilder<MySql> = QueryBuilder::new(format!("SELECT COUNT(*) AS total {base}"));
        count.push_bind(&desde).push(" AND ").push_bind(&hasta).push(" AND b.accion NOT LIKE 'API %') t");
        if let Some(c) = &categoria {
            count.push(" WHERE t.categoria = ").push_bind(c);
        }
        let total = row_get_i64(
            &count.build().fetch_one(pool).await.map_err(AppError::Database)?,
            "total",
        );

        let mut sel: QueryBuilder<MySql> = QueryBuilder::new(format!(
            "SELECT DATE_FORMAT(t.fecha_hora, '%Y-%m-%d %H:%i:%s') AS fecha_hora, \
                    CONCAT(u.nombre, ' ', COALESCE(u.apellido_paterno, '')) AS usuario, \
                    COALESCE(r.nombre_rol, 'Sin Rol') AS rol, t.accion, t.detalle, t.categoria {base}"
        ));
        sel.push_bind(&desde).push(" AND ").push_bind(&hasta).push(" AND b.accion NOT LIKE 'API %') t ");
        sel.push("JOIN usuarios u ON t.id_usuarios = u.id_usuarios LEFT JOIN roles r ON u.id_roles = r.id_roles ");
        if let Some(c) = &categoria {
            sel.push("WHERE t.categoria = ").push_bind(c);
        }
        sel.push(" ORDER BY t.fecha_hora DESC LIMIT ")
            .push_bind(per_page as i64)
            .push(" OFFSET ")
            .push_bind((page as i64) * (per_page as i64));

        let rows = sel.build().fetch_all(pool).await.map_err(AppError::Database)?;
        let items = rows
            .into_iter()
            .map(|r| BitacoraItem {
                fecha_hora: r.get("fecha_hora"),
                usuario: r.get("usuario"),
                rol: r.get("rol"),
                accion: r.get("accion"),
                detalle: r.try_get::<Option<String>, _>("detalle").ok().flatten().unwrap_or_default(),
                categoria: r.get("categoria"),
            })
            .collect();

        Ok(BitacoraPagina { items, total, page, per_page })
    }
}
