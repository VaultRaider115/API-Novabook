use super::producto::TipoProducto;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoMovimiento {
    BodegaATienda,
    TiendaABodega,
}

impl TipoMovimiento {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BodegaATienda => "BODEGA_A_TIENDA",
            Self::TiendaABodega => "TIENDA_A_BODEGA",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Movimiento {
    pub id_movimiento: i64,
    pub tipo_movimiento: TipoMovimiento,
    pub tipo_producto: TipoProducto,
    pub codigo_ean: i64,
    pub cantidad: i32,
    pub origen_ubicacion: i32,
    pub destino_ubicacion: i32,
    pub id_usuario: i64,
    pub fecha_hora: String,
    pub observaciones: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TrasladoRequest {
    pub codigo_ean: i64,
    pub cantidad: i32,
    pub observaciones: Option<String>,
}

/// Una partida de una requisición (documento con varios productos).
#[derive(Debug, Clone, Deserialize)]
pub struct PartidaRequisicion {
    pub codigo_ean: i64,
    pub cantidad: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RequisicionRequest {
    pub items: Vec<PartidaRequisicion>,
    pub observaciones: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PartidaRequisicionResp {
    pub codigo_ean: i64,
    pub tipo_producto: String,
    pub titulo: String,
    pub cantidad: i32,
}

/// Requisición registrada: todas sus partidas comparten folio (REQ-AAAA-n).
#[derive(Debug, Clone, Serialize)]
pub struct RequisicionResponse {
    pub folio: String,
    pub fecha_hora: String,
    pub tipo_movimiento: TipoMovimiento,
    pub total_piezas: i32,
    /// Editorial (proveedor) del documento: una sola por requisición.
    pub editorial: String,
    pub items: Vec<PartidaRequisicionResp>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrasladoResponse {
    pub codigo_ean: i64,
    pub tipo_producto: TipoProducto,
    pub tipo_movimiento: TipoMovimiento,
    pub cantidad_trasladada: i32,
    pub stock_origen_restante: i32,
    pub stock_destino_nuevo: i32,
    pub mensaje: String,
}

// ----------------------------------------------------------------------------
// Recepción de la requisición de salida (diagrama, pasos 2 y 3): el Vendedor
// confirma lo que llegó (o lo corrige) y elige el mueble de piso de cada libro.
// ----------------------------------------------------------------------------

/// Filtros de GET /requisiciones (`tipo` SALIDA|ENTRADA, `estado`).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct FiltroRequisiciones {
    pub tipo: Option<String>,
    pub estado: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequisicionResumen {
    pub folio: String,
    /// SALIDA (bodega → piso) | ENTRADA (piso → bodega)
    pub tipo: String,
    /// PENDIENTE | CONFIRMADA | CORREGIDA | REGISTRADA
    pub estado: String,
    pub fecha_hora: String,
    pub editorial: String,
    pub registro: String,
    pub recibio: Option<String>,
    pub fecha_recepcion: Option<String>,
    pub observaciones: Option<String>,
    pub partidas: i32,
    pub total_piezas: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct PartidaRequisicionDetalle {
    pub codigo_ean: i64,
    pub tipo_producto: String,
    pub titulo: String,
    /// Lo que dice el documento original.
    pub cantidad: i32,
    /// Lo que confirmó el Vendedor (None mientras esté pendiente).
    pub cantidad_recibida: Option<i32>,
    pub id_mueble: i32,
    pub mueble: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequisicionDetalle {
    #[serde(flatten)]
    pub resumen: RequisicionResumen,
    pub items: Vec<PartidaRequisicionDetalle>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PartidaRecepcion {
    pub codigo_ean: i64,
    pub cantidad_recibida: i32,
    /// Mueble del piso de venta donde quedó el libro.
    pub id_mueble: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RecepcionRequest {
    pub items: Vec<PartidaRecepcion>,
    pub observaciones: Option<String>,
}
