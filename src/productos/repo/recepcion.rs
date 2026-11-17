//! Requisiciones como documento (tabla `requisicion`) y su recepción en piso.
//!
//! Diagrama de procesos, pasos 2 y 3: Bodega registra la salida (folio
//! REQ-AAAA-n) y el Vendedor la recibe. Si todo coincide la confirma; si no,
//! captura lo que llegó y la requisición queda CORREGIDA: el stock y las filas
//! de `movimientos` del folio se ajustan a lo recibido, de modo que el
//! documento reimpreso y la regla de la mitad (paso 6) usan la cantidad real.
//! Además el Vendedor elige el mueble de piso donde quedó cada libro.

use sqlx::Row;

use crate::{
    db::{DbPool, row_get_i32, row_get_i64},
    error::AppError,
    productos::{
        model::{
            FiltroRequisiciones, PartidaRequisicionDetalle, RecepcionRequest, RequisicionDetalle,
            RequisicionResumen,
        },
        repo::{
            UBICACION_BODEGA, UBICACION_TIENDA,
            stock::{self, Tx},
        },
    },
};

macro_rules! select_resumen {
    () => {
        "SELECT r.folio, r.tipo, r.estado, \
        DATE_FORMAT(r.fecha, '%Y-%m-%d %H:%i:%s') AS fecha_hora, \
        COALESCE(p.nombre_proveedor, '') AS editorial, \
        TRIM(CONCAT(u.nombre, ' ', COALESCE(u.apellido_paterno, ''))) AS registro, \
        TRIM(CONCAT(ur.nombre, ' ', COALESCE(ur.apellido_paterno, ''))) AS recibio, \
        DATE_FORMAT(r.fecha_recepcion, '%Y-%m-%d %H:%i:%s') AS fecha_recepcion, \
        r.observaciones, \
        (SELECT COUNT(*) FROM detalle_requisicion d WHERE d.folio = r.folio) AS partidas, \
        (SELECT COALESCE(SUM(COALESCE(d.cantidad_recibida, d.cantidad)), 0) \
           FROM detalle_requisicion d WHERE d.folio = r.folio) AS total_piezas \
     FROM requisicion r \
     LEFT JOIN proveedor p ON p.id_proveedor = r.id_proveedor \
     JOIN usuarios u ON u.id_usuarios = r.id_usuarios \
     LEFT JOIN usuarios ur ON ur.id_usuarios = r.id_usuario_recibe"
    };
}
const SELECT_RESUMEN: &str = select_resumen!();

/// Condición SQL para las filas de `movimientos` de un folio. La nota es
/// "Requisición REQ-AAAA-n" o "Requisición REQ-AAAA-n · <observaciones>";
/// se compara exacto para que REQ-2026-1 no atrape a REQ-2026-15.
macro_rules! cond_folio {
    () => {
        "(observaciones = ? OR observaciones LIKE CONCAT(?, ' · %'))"
    };
}

fn resumen_de(r: &sqlx::mysql::MySqlRow) -> RequisicionResumen {
    let recibio: Option<String> = r.try_get("recibio").ok().flatten();
    RequisicionResumen {
        folio: r.get("folio"),
        tipo: r.get("tipo"),
        estado: r.get("estado"),
        fecha_hora: r.get("fecha_hora"),
        editorial: r.get("editorial"),
        registro: r.get("registro"),
        recibio: recibio.filter(|s| !s.is_empty()),
        fecha_recepcion: r.try_get("fecha_recepcion").ok().flatten(),
        observaciones: r.try_get("observaciones").ok().flatten(),
        partidas: row_get_i32(r, "partidas"),
        total_piezas: row_get_i32(r, "total_piezas"),
    }
}

/// Partida de la petición de recepción, ya emparejada con el documento.
struct Partida {
    codigo_ean: i64,
    enviada: i32,
    mueble_original: i32,
    recibida: i32,
    mueble_nuevo: i32,
}

/// Empareja lo capturado con las partidas del documento: deben venir todas,
/// una vez cada una, sin EAN ajenos y sin cantidades negativas.
fn emparejar(documento: &[(i64, i32, i32)], req: &RecepcionRequest) -> Result<Vec<Partida>, AppError> {
    let mut vistos = std::collections::HashSet::new();
    for it in &req.items {
        if !documento.iter().any(|(ean, _, _)| *ean == it.codigo_ean) {
            return Err(AppError::BadRequest(format!(
                "El EAN {} no viene en esta requisición; si llegó de más, Bodega debe hacer otra requisición",
                it.codigo_ean
            )));
        }
        if !vistos.insert(it.codigo_ean) {
            return Err(AppError::BadRequest(format!("El EAN {} viene repetido", it.codigo_ean)));
        }
        if it.cantidad_recibida < 0 {
            return Err(AppError::BadRequest(format!(
                "La cantidad recibida del EAN {} no puede ser negativa", it.codigo_ean
            )));
        }
    }
    documento
        .iter()
        .map(|(ean, enviada, mueble)| {
            let it = req.items.iter().find(|i| i.codigo_ean == *ean).ok_or_else(|| {
                AppError::BadRequest(format!("Falta capturar lo recibido del EAN {}", ean))
            })?;
            Ok(Partida {
                codigo_ean: *ean,
                enviada: *enviada,
                mueble_original: *mueble,
                recibida: it.cantidad_recibida,
                mueble_nuevo: it.id_mueble,
            })
        })
        .collect()
}

pub struct RecepcionRepo;

impl RecepcionRepo {
    pub async fn listar(pool: &DbPool, f: &FiltroRequisiciones) -> Result<Vec<RequisicionResumen>, AppError> {
        let mut q = sqlx::QueryBuilder::<sqlx::MySql>::new(SELECT_RESUMEN);
        q.push(" WHERE 1 = 1");
        if let Some(t) = f.tipo.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
            q.push(" AND r.tipo = ").push_bind(t.to_uppercase());
        }
        if let Some(e) = f.estado.as_deref().map(str::trim).filter(|e| !e.is_empty()) {
            q.push(" AND r.estado = ").push_bind(e.to_uppercase());
        }
        q.push(" ORDER BY r.fecha DESC, r.folio DESC LIMIT 200");
        let rows = q.build().fetch_all(pool).await.map_err(AppError::Database)?;
        Ok(rows.iter().map(resumen_de).collect())
    }

    pub async fn obtener(pool: &DbPool, folio: &str) -> Result<RequisicionDetalle, AppError> {
        let fila = sqlx::query(concat!(select_resumen!(), " WHERE r.folio = ?"))
            .bind(folio)
            .fetch_optional(pool)
            .await
            .map_err(AppError::Database)?
            .ok_or_else(|| AppError::NotFound(format!("No existe la requisición {}", folio)))?;
        let items = sqlx::query(
            "SELECT d.codigo_ean, d.cantidad, d.cantidad_recibida, d.id_mueble, pr.tipo_producto, \
                    COALESCE(l.nombre_libro, rv.nombre_revista, '') AS titulo, \
                    CONCAT(m.codigo_mueble, ' · ', m.nombre_mueble) AS mueble \
             FROM detalle_requisicion d \
             JOIN productos pr ON pr.codigo_ean = d.codigo_ean \
             JOIN mueble m ON m.id_mueble = d.id_mueble \
             LEFT JOIN libros l ON l.codigo_ean = d.codigo_ean \
             LEFT JOIN revistas rv ON rv.codigo_ean = d.codigo_ean \
             WHERE d.folio = ? ORDER BY titulo",
        )
        .bind(folio)
        .fetch_all(pool)
        .await
        .map_err(AppError::Database)?
        .iter()
        .map(|r| PartidaRequisicionDetalle {
            codigo_ean: row_get_i64(r, "codigo_ean"),
            tipo_producto: r.get("tipo_producto"),
            titulo: r.get("titulo"),
            cantidad: row_get_i32(r, "cantidad"),
            cantidad_recibida: crate::db::row_get_opt_i32(r, "cantidad_recibida"),
            id_mueble: row_get_i32(r, "id_mueble"),
            mueble: r.get("mueble"),
        })
        .collect();
        Ok(RequisicionDetalle { resumen: resumen_de(&fila), items })
    }

    /// Recibe en piso una requisición de salida. Devuelve las diferencias
    /// (para la bitácora) y los cambios de mueble.
    pub async fn recibir(
        pool: &DbPool,
        folio: &str,
        actor_id: i64,
        req: &RecepcionRequest,
    ) -> Result<(Vec<String>, Vec<String>), AppError> {
        let mut tx = pool.begin().await.map_err(AppError::Database)?;

        let doc = sqlx::query(
            "SELECT tipo, estado, id_usuarios, observaciones, DATE_FORMAT(fecha, '%Y-%m-%d %H:%i:%s') AS fecha \
             FROM requisicion WHERE folio = ? FOR UPDATE",
        )
        .bind(folio)
        .fetch_optional(&mut *tx)
        .await
        .map_err(AppError::Database)?
        .ok_or_else(|| AppError::NotFound(format!("No existe la requisición {}", folio)))?;
        let tipo: String = doc.get("tipo");
        let estado: String = doc.get("estado");
        if tipo != "SALIDA" {
            return Err(AppError::BadRequest(
                "Solo se reciben en piso las requisiciones de salida (bodega → piso)".to_string(),
            ));
        }
        if estado != "PENDIENTE" {
            return Err(AppError::Conflict(format!(
                "La requisición {} ya fue recibida ({})", folio, estado
            )));
        }
        let creador = row_get_i64(&doc, "id_usuarios");
        let fecha_doc: String = doc.get("fecha");
        let obs_doc: Option<String> = doc.try_get("observaciones").ok().flatten();

        let documento: Vec<(i64, i32, i32)> = sqlx::query(
            "SELECT codigo_ean, cantidad, id_mueble FROM detalle_requisicion WHERE folio = ? FOR UPDATE",
        )
        .bind(folio)
        .fetch_all(&mut *tx)
        .await
        .map_err(AppError::Database)?
        .iter()
        .map(|r| (row_get_i64(r, "codigo_ean"), row_get_i32(r, "cantidad"), row_get_i32(r, "id_mueble")))
        .collect();
        let partidas = emparejar(&documento, req)?;

        let base = format!("Requisición {}", folio);
        let mut diferencias = Vec::new();
        let mut acomodos = Vec::new();

        for p in &partidas {
            let codigo_nuevo = Self::mueble_de_piso(&mut tx, p.mueble_nuevo).await?;

            if p.recibida < p.enviada {
                Self::regresar_faltante(&mut tx, &base, p).await?;
            } else if p.recibida > p.enviada {
                Self::bajar_sobrante(&mut tx, &base, p, creador, &fecha_doc).await?;
            }
            if p.recibida != p.enviada {
                diferencias.push(format!("EAN {} {}→{}", p.codigo_ean, p.enviada, p.recibida));
            }

            if p.mueble_nuevo != p.mueble_original {
                Self::acomodar(&mut tx, &base, p, &codigo_nuevo).await?;
                acomodos.push(format!("EAN {} → {}", p.codigo_ean, codigo_nuevo));
            }

            sqlx::query(
                "UPDATE detalle_requisicion SET cantidad_recibida = ?, id_mueble = ? WHERE folio = ? AND codigo_ean = ?",
            )
            .bind(p.recibida)
            .bind(p.mueble_nuevo)
            .bind(folio)
            .bind(p.codigo_ean)
            .execute(&mut *tx)
            .await
            .map_err(AppError::Database)?;
        }

        let nuevo_estado = if diferencias.is_empty() { "CONFIRMADA" } else { "CORREGIDA" };
        let nota_recepcion = req.observaciones.as_deref().map(str::trim).filter(|o| !o.is_empty());
        let observaciones: Option<String> = match (obs_doc, nota_recepcion) {
            (Some(o), Some(n)) => Some(format!("{} · Recepción: {}", o, n)),
            (None, Some(n)) => Some(format!("Recepción: {}", n)),
            (o, None) => o,
        }
        .map(|o| o.chars().take(200).collect());

        sqlx::query(
            "UPDATE requisicion SET estado = ?, id_usuario_recibe = ?, fecha_recepcion = NOW(), observaciones = ? \
             WHERE folio = ?",
        )
        .bind(nuevo_estado)
        .bind(actor_id)
        .bind(observaciones)
        .bind(folio)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Database)?;

        tx.commit().await.map_err(AppError::Database)?;
        Ok((diferencias, acomodos))
    }

    /// Valida que el mueble sea del piso de venta y devuelve su código.
    async fn mueble_de_piso(tx: &mut Tx<'_>, id_mueble: i32) -> Result<String, AppError> {
        let fila = sqlx::query("SELECT codigo_mueble FROM mueble WHERE id_mueble = ? AND id_ubicacion = ?")
            .bind(id_mueble)
            .bind(UBICACION_TIENDA)
            .fetch_optional(&mut **tx)
            .await
            .map_err(AppError::Database)?
            .ok_or_else(|| {
                AppError::BadRequest(format!("El mueble {} no es un mueble del piso de venta", id_mueble))
            })?;
        Ok(fila.get("codigo_mueble"))
    }

    /// Llegaron menos: lo que falta regresa a su mueble de bodega y las filas
    /// del folio bajan a la cantidad real.
    async fn regresar_faltante(tx: &mut Tx<'_>, base: &str, p: &Partida) -> Result<(), AppError> {
        let faltan = p.enviada - p.recibida;
        let hay = stock::en_mueble_bloqueado(tx, p.codigo_ean, p.mueble_original).await?;
        if hay < faltan {
            return Err(AppError::Conflict(format!(
                "Del EAN {} solo quedan {} pieza(s) en el mueble de piso donde se registró la salida; \
                 no se pueden regresar {} a bodega",
                p.codigo_ean, hay, faltan
            )));
        }
        stock::descontar_de_mueble(tx, p.codigo_ean, p.mueble_original, faltan).await?;

        let filas = sqlx::query(concat!(
            "SELECT id_movimientos, mueble_origen, cantidad FROM movimientos \
             WHERE codigo_ean = ? AND mueble_destino = ? AND ", cond_folio!(), " ORDER BY cantidad DESC FOR UPDATE"
        ))
        .bind(p.codigo_ean)
        .bind(p.mueble_original)
        .bind(base)
        .bind(base)
        .fetch_all(&mut **tx)
        .await
        .map_err(AppError::Database)?;

        let mut pendiente = faltan;
        for f in filas {
            if pendiente == 0 {
                break;
            }
            let id = row_get_i64(&f, "id_movimientos");
            let origen = row_get_i32(&f, "mueble_origen");
            let quita = row_get_i32(&f, "cantidad").min(pendiente);
            sqlx::query("UPDATE movimientos SET cantidad = cantidad - ? WHERE id_movimientos = ?")
                .bind(quita)
                .bind(id)
                .execute(&mut **tx)
                .await
                .map_err(AppError::Database)?;
            stock::sumar(tx, p.codigo_ean, origen, quita).await?;
            pendiente -= quita;
        }
        // Sin filas que ajustar (no debería pasar): a cualquier mueble de bodega.
        if pendiente > 0 {
            let m = stock::elegir_mueble(tx, p.codigo_ean, UBICACION_BODEGA, pendiente).await?;
            stock::sumar(tx, p.codigo_ean, m, pendiente).await?;
        }
        sqlx::query(concat!("DELETE FROM movimientos WHERE codigo_ean = ? AND cantidad = 0 AND ", cond_folio!(), ""))
            .bind(p.codigo_ean)
            .bind(base)
            .bind(base)
            .execute(&mut **tx)
            .await
            .map_err(AppError::Database)?;
        Ok(())
    }

    /// Llegaron más: el sobrante sale de bodega dentro del mismo documento
    /// (misma fecha y nota, para que cuente en la misma bajada).
    async fn bajar_sobrante(
        tx: &mut Tx<'_>,
        base: &str,
        p: &Partida,
        creador: i64,
        fecha_doc: &str,
    ) -> Result<(), AppError> {
        let extra = p.recibida - p.enviada;
        let hay = stock::stock_bloqueado(tx, p.codigo_ean, UBICACION_BODEGA).await?;
        if hay < extra {
            return Err(AppError::BadRequest(format!(
                "Del EAN {} llegaron {} de más, pero en bodega solo hay {}; revisa el conteo",
                p.codigo_ean, extra, hay
            )));
        }
        let usados = stock::descontar(tx, p.codigo_ean, UBICACION_BODEGA, extra).await?;
        stock::sumar(tx, p.codigo_ean, p.mueble_original, extra).await?;

        let ref_fila = sqlx::query(concat!(
            "SELECT DATE_FORMAT(fecha_hora, '%Y-%m-%d %H:%i:%s') AS fecha, observaciones FROM movimientos \
             WHERE ", cond_folio!(), " ORDER BY id_movimientos LIMIT 1"
        ))
        .bind(base)
        .bind(base)
        .fetch_optional(&mut **tx)
        .await
        .map_err(AppError::Database)?;
        let (fecha, nota) = match ref_fila {
            Some(f) => (f.get::<String, _>("fecha"), f.get::<Option<String>, _>("observaciones").unwrap_or_else(|| base.to_string())),
            None => (fecha_doc.to_string(), base.to_string()),
        };

        for (origen, piezas) in usados {
            let upd = sqlx::query(concat!(
                "UPDATE movimientos SET cantidad = cantidad + ? \
                 WHERE codigo_ean = ? AND mueble_origen = ? AND mueble_destino = ? AND ", cond_folio!(), " LIMIT 1"
            ))
            .bind(piezas)
            .bind(p.codigo_ean)
            .bind(origen)
            .bind(p.mueble_original)
            .bind(base)
            .bind(base)
            .execute(&mut **tx)
            .await
            .map_err(AppError::Database)?;
            if upd.rows_affected() == 0 {
                sqlx::query(
                    "INSERT INTO movimientos (id_usuarios, codigo_ean, mueble_origen, mueble_destino, cantidad, fecha_hora, observaciones) \
                     VALUES (?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(creador)
                .bind(p.codigo_ean)
                .bind(origen)
                .bind(p.mueble_original)
                .bind(piezas)
                .bind(&fecha)
                .bind(&nota)
                .execute(&mut **tx)
                .await
                .map_err(AppError::Database)?;
            }
        }
        Ok(())
    }

    /// Pasa lo recibido al mueble de piso que eligió el Vendedor. Si ya se
    /// vendió algo desde la salida, solo mueve lo que queda.
    async fn acomodar(tx: &mut Tx<'_>, base: &str, p: &Partida, codigo_nuevo: &str) -> Result<(), AppError> {
        let hay = stock::en_mueble_bloqueado(tx, p.codigo_ean, p.mueble_original).await?;
        let mover = hay.min(p.recibida);
        if mover > 0 {
            if let Some(libre) = stock::libre_en_mueble(tx, p.mueble_nuevo).await?
                && libre < mover
            {
                return Err(AppError::Conflict(format!(
                    "El mueble {} solo tiene espacio para {} pieza(s) y el EAN {} necesita {}",
                    codigo_nuevo, libre.max(0), p.codigo_ean, mover
                )));
            }
            stock::descontar_de_mueble(tx, p.codigo_ean, p.mueble_original, mover).await?;
            stock::sumar(tx, p.codigo_ean, p.mueble_nuevo, mover).await?;
        }
        sqlx::query(concat!(
            "UPDATE movimientos SET mueble_destino = ? WHERE codigo_ean = ? AND mueble_destino = ? AND ", cond_folio!(), ""
        ))
        .bind(p.mueble_nuevo)
        .bind(p.codigo_ean)
        .bind(p.mueble_original)
        .bind(base)
        .bind(base)
        .execute(&mut **tx)
        .await
        .map_err(AppError::Database)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::emparejar;
    use crate::productos::model::{PartidaRecepcion, RecepcionRequest};

    fn req(items: &[(i64, i32, i32)]) -> RecepcionRequest {
        RecepcionRequest {
            items: items
                .iter()
                .map(|&(codigo_ean, cantidad_recibida, id_mueble)| PartidaRecepcion { codigo_ean, cantidad_recibida, id_mueble })
                .collect(),
            observaciones: None,
        }
    }

    const DOC: [(i64, i32, i32); 2] = [(9781, 16, 11), (9782, 4, 12)];

    #[test]
    fn empareja_todas_las_partidas() {
        let p = emparejar(&DOC, &req(&[(9782, 4, 13), (9781, 14, 11)])).unwrap();
        assert_eq!(p.len(), 2);
        assert_eq!((p[0].codigo_ean, p[0].enviada, p[0].recibida, p[0].mueble_nuevo), (9781, 16, 14, 11));
        assert_eq!((p[1].mueble_original, p[1].mueble_nuevo), (12, 13));
    }

    #[test]
    fn rechaza_faltantes_repetidos_ajenos_y_negativos() {
        assert!(emparejar(&DOC, &req(&[(9781, 16, 11)])).is_err());
        assert!(emparejar(&DOC, &req(&[(9781, 16, 11), (9781, 1, 11), (9782, 4, 12)])).is_err());
        assert!(emparejar(&DOC, &req(&[(9781, 16, 11), (9782, 4, 12), (9783, 1, 11)])).is_err());
        assert!(emparejar(&DOC, &req(&[(9781, -1, 11), (9782, 4, 12)])).is_err());
    }
}
