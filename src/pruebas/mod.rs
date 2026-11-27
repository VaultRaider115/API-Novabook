//! Pruebas unitarias por caso de uso (hito 3).
//!
//! Cada módulo `cuNN_*` prueba un caso de uso del diagrama v4 llamando al
//! router real de la API dentro del proceso (sin red): login con JWT, guardas
//! por rol, controlador, servicio, repositorio y base de datos.
//!
//! La base es `Libreria_pruebas`, que se crea desde
//! `libreria_unificada-definitivo.sql` una sola vez por corrida de
//! `cargo test`; la base `Libreria` de la app nunca se toca. Requiere MariaDB
//! encendido (XAMPP) con el usuario de `DATABASE_URL`.
//!
//! Formato de la documentación (lo lee `docs/pruebas/generar_documento.py`):
//! cada prueba lleva encima tres líneas `///`:
//!   /// PU-NN-MM · Título de la prueba
//!   /// Entrada: datos de entrada
//!   /// Esperado: resultado esperado

#![allow(dead_code)]

mod cu01_login;
mod cu02_usuarios;
mod cu03_autores;
mod cu04_generos;
mod cu05_muebles;
mod cu06_motivos;
mod cu07_registrar_libros;
mod cu08_registrar_revistas;
mod cu09_compra_proveedor;
mod cu10_consultar_existencias;
mod cu11_bodega_tienda_libros;
mod cu12_bodega_tienda_revistas;
mod cu13_tienda_bodega_libros;
mod cu14_tienda_bodega_revistas;
mod cu15_ventas;
mod cu16_aprobar_devoluciones;
mod cu17_pdf_libros;
mod cu18_pdf_revistas;
mod cu19_movimiento_diario;
mod cu20_gestionar_usuarios_jefe;
mod cu21_historial_devoluciones;
mod cu22_devolucion_cliente;
mod cu23_levantar_inventario;
mod cu24_revisar_inventario;
mod cu25_solicitudes_editorial;
mod cu26_mis_devoluciones;
mod cu27_proveedores_editoriales;
mod cu28_cierre_sesion;
mod cu29_incidencia_compra;
mod cu30_recibir_requisicion;
mod cu31_devolucion_proveedor;
mod cu32_tickets_venta;

use std::sync::atomic::{AtomicI64, Ordering};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
};
use serde_json::{Value, json};
use sqlx::{
    Connection, Executor,
    mysql::{MySqlConnectOptions, MySqlConnection, MySqlPoolOptions},
};
use std::str::FromStr;
use tokio::sync::{OnceCell, RwLock, RwLockReadGuard, RwLockWriteGuard};
use tower::ServiceExt;

use crate::{
    config::Config,
    db::{AppState, DbPool},
};

pub const BASE_PRUEBAS: &str = "Libreria_pruebas";
pub const SECRETO_JWT: &str = "secreto-solo-para-pruebas-unitarias-novabook";

/// Género y muebles propios de las pruebas (capacidad de sobra): los productos
/// de prueba van a ese género, así que sus movimientos no llenan los muebles
/// de la semilla.
pub const GENERO_PRUEBAS: i32 = 900;
pub const MUEBLE_PISO: i32 = 900;
pub const MUEBLE_BODEGA: i32 = 901;

/// Usuarios de la semilla.
pub const GERENTE: (i64, &str) = (1001, "admin123");
pub const JEFE: (i64, &str) = (350976899, "2501");
pub const BODEGA: (i64, &str) = (628777130, "7777");
pub const VENDEDOR: (i64, &str) = (628777129, "8888");

/// Proveedores de la semilla (`productos.id_proveedor` hace de editorial).
pub const PLANETA: i32 = 1;
pub const PLANETA_INFANTIL: i32 = 2;
pub const PENGUIN: i32 = 3;

// ---------------------------------------------------------------------------
// Base de pruebas
// ---------------------------------------------------------------------------

static BASE_LISTA: OnceCell<()> = OnceCell::const_new();

/// Las pruebas con base de datos corren UNA A LA VEZ para que cada una mida
/// solo su regla y el resultado no dependa del orden: cerrar y aprobar un
/// inventario mensual ajusta el piso de TODOS los productos, y los traslados
/// simultáneos en sentidos opuestos pueden chocar en MariaDB (bloqueo mutuo,
/// error 1213; ver PU-13-07, que sí prueba la concurrencia a propósito).
static CANDADO: RwLock<()> = RwLock::const_new(());

fn url_base() -> String {
    dotenvy::dotenv().ok();
    std::env::var("DATABASE_URL_PRUEBAS")
        .ok()
        .or_else(|| std::env::var("DATABASE_URL").ok().map(|u| con_base(&u, BASE_PRUEBAS)))
        .unwrap_or_else(|| format!("mysql://root@127.0.0.1:3306/{BASE_PRUEBAS}"))
}

/// Cambia el nombre de la base de una URL `mysql://usuario@host:puerto/base?x`.
fn con_base(url: &str, base: &str) -> String {
    let inicio = url.find("://").map(|i| i + 3).unwrap_or(0);
    let sin_query = url.split('?').next().unwrap_or(url);
    match sin_query[inicio..].find('/') {
        Some(i) => format!("{}/{}", &sin_query[..inicio + i], base),
        None => format!("{}/{}", sin_query, base),
    }
}

async fn preparar_base() {
    let url = url_base();
    assert!(
        url.ends_with(&format!("/{BASE_PRUEBAS}")),
        "Por seguridad las pruebas solo usan la base {BASE_PRUEBAS} (URL: {})",
        crate::db::url_sin_contrasena(&url)
    );
    let servidor = con_base(&url, "mysql");
    let mut con = MySqlConnection::connect(&servidor).await.unwrap_or_else(|e| {
        panic!(
            "No hay conexión con MariaDB ({}): enciende MySQL en XAMPP. {e}",
            crate::db::url_sin_contrasena(&servidor)
        )
    });
    let esquema = include_str!("../../libreria_unificada-definitivo.sql")
        .replace("`Libreria`", &format!("`{BASE_PRUEBAS}`"))
        .replace("'Libreria'", &format!("'{BASE_PRUEBAS}'"));
    con.execute(sqlx::AssertSqlSafe(format!("DROP DATABASE IF EXISTS `{BASE_PRUEBAS}`")))
        .await
        .expect("no se pudo borrar la base de pruebas anterior");
    sqlx::raw_sql(sqlx::AssertSqlSafe(esquema))
        .execute(&mut con)
        .await
        .expect("no se pudo importar el esquema en la base de pruebas");
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "USE `{BASE_PRUEBAS}`; \
         INSERT INTO genero (id_genero, genero_literario) VALUES ({GENERO_PRUEBAS}, 'Pruebas unitarias'); \
         INSERT INTO mueble (id_mueble, id_genero, id_ubicacion, codigo_mueble, nombre_mueble, capacidad) VALUES \
           ({MUEBLE_PISO}, {GENERO_PRUEBAS}, 1, 'P-900', 'Piso de pruebas', 1000000), \
           ({MUEBLE_BODEGA}, {GENERO_PRUEBAS}, 2, 'B-900', 'Bodega de pruebas', 1000000);"
    )))
    .execute(&mut con)
    .await
    .expect("no se pudieron crear el género y los muebles de pruebas");
    con.close().await.ok();
}

// ---------------------------------------------------------------------------
// Contexto de cada prueba
// ---------------------------------------------------------------------------

pub struct Ctx {
    pub app: Router,
    pub pool: DbPool,
    _compartido: Option<RwLockReadGuard<'static, ()>>,
    _exclusivo: Option<RwLockWriteGuard<'static, ()>>,
}

pub struct Resp {
    pub status: StatusCode,
    pub json: Value,
    pub bytes: Vec<u8>,
    pub content_type: String,
}

impl Resp {
    pub fn ok(&self) -> bool {
        self.status.is_success() && self.json["success"] != json!(false)
    }
    pub fn data(&self) -> &Value {
        &self.json["data"]
    }
    pub fn mensaje(&self) -> String {
        self.json["error"]["message"]
            .as_str()
            .or_else(|| self.json["message"].as_str())
            .unwrap_or_default()
            .to_string()
    }
    /// Falla con el cuerpo completo si la respuesta no fue exitosa.
    pub fn exito(self) -> Self {
        assert!(self.ok(), "se esperaba éxito y respondió {}: {}", self.status, self.json);
        self
    }
    /// Comprueba el código HTTP y que el mensaje contenga `texto`.
    pub fn error(self, status: StatusCode, texto: &str) -> Self {
        assert_eq!(self.status, status, "código inesperado; respuesta: {}", self.json);
        assert!(
            self.mensaje().contains(texto),
            "el mensaje no contiene {texto:?}: {:?}",
            self.mensaje()
        );
        self
    }
}

/// Contexto de una prueba: base de pruebas lista, router y turno exclusivo.
pub async fn ctx() -> Ctx {
    ctx_exclusivo().await
}

/// Igual que `ctx()`; se deja con nombre propio en el inventario mensual para
/// que quede claro que esas pruebas no pueden compartir la base con otras.
pub async fn ctx_exclusivo() -> Ctx {
    let guardia = CANDADO.write().await;
    let mut c = construir().await;
    c._exclusivo = Some(guardia);
    c
}

async fn construir() -> Ctx {
    BASE_LISTA.get_or_init(preparar_base).await;
    let opciones = MySqlConnectOptions::from_str(&url_base()).expect("URL de la base de pruebas inválida");
    let pool = MySqlPoolOptions::new()
        .max_connections(4)
        .min_connections(0)
        .acquire_timeout(std::time::Duration::from_secs(30))
        .connect_with(opciones)
        .await
        .expect("no se pudo abrir el pool de la base de pruebas");
    let config = Config {
        database_url: url_base(),
        jwt_secret: SECRETO_JWT.to_string(),
        jwt_expiration_hours: 1,
        server_host: "127.0.0.1".to_string(),
        server_port: 0,
    };
    let state = AppState { pool: pool.clone(), config };
    let app = Router::new()
        .merge(crate::usuarios::controller::router(state.clone()))
        .merge(crate::productos::controller::router(state.clone()))
        .with_state(state);
    Ctx { app, pool, _compartido: None, _exclusivo: None }
}

impl Ctx {
    pub async fn pedir(&self, metodo: Method, uri: &str, token: Option<&str>, cuerpo: Option<Value>) -> Resp {
        let mut req = Request::builder().method(metodo).uri(uri);
        if let Some(t) = token {
            req = req.header(header::AUTHORIZATION, format!("Bearer {t}"));
        }
        let body = match cuerpo {
            Some(v) => {
                req = req.header(header::CONTENT_TYPE, "application/json");
                Body::from(v.to_string())
            }
            None => Body::empty(),
        };
        let resp = self.app.clone().oneshot(req.body(body).unwrap()).await.unwrap();
        let status = resp.status();
        let content_type = resp
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string();
        let bytes = to_bytes(resp.into_body(), 50 * 1024 * 1024).await.unwrap().to_vec();
        let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        Resp { status, json, bytes, content_type }
    }

    pub async fn get(&self, token: &str, uri: &str) -> Resp {
        self.pedir(Method::GET, uri, Some(token), None).await
    }
    pub async fn post(&self, token: &str, uri: &str, cuerpo: Value) -> Resp {
        self.pedir(Method::POST, uri, Some(token), Some(cuerpo)).await
    }
    pub async fn put(&self, token: &str, uri: &str, cuerpo: Value) -> Resp {
        self.pedir(Method::PUT, uri, Some(token), Some(cuerpo)).await
    }
    pub async fn delete(&self, token: &str, uri: &str) -> Resp {
        self.pedir(Method::DELETE, uri, Some(token), None).await
    }

    pub async fn login(&self, (id, contrasena): (i64, &str)) -> Resp {
        self.pedir(
            Method::POST,
            "/api/auth/login",
            None,
            Some(json!({ "id_usuario": id, "contrasena": contrasena })),
        )
        .await
    }

    /// Token JWT de un usuario (falla si no puede iniciar sesión).
    pub async fn token(&self, usuario: (i64, &str)) -> String {
        let r = self.login(usuario).await.exito();
        r.data()["token"].as_str().expect("el login no devolvió token").to_string()
    }

    // ----- Consultas directas para verificar el efecto en la base -----

    pub async fn escalar_i64(&self, sql: &'static str, ean: i64) -> i64 {
        let fila = sqlx::query(sql).bind(ean).fetch_one(&self.pool).await.unwrap();
        crate::db::row_get_i64(&fila, "n")
    }

    /// Piezas de un producto en piso (ubicación 1) o bodega (2).
    pub async fn stock(&self, ean: i64, ubicacion: i32) -> i64 {
        let fila = sqlx::query(
            "SELECT COALESCE(SUM(e.cantidad), 0) AS n FROM existencias e \
             JOIN mueble m ON m.id_mueble = e.id_mueble WHERE e.codigo_ean = ? AND m.id_ubicacion = ?",
        )
        .bind(ean)
        .bind(ubicacion)
        .fetch_one(&self.pool)
        .await
        .unwrap();
        crate::db::row_get_i64(&fila, "n")
    }
    pub async fn piso(&self, ean: i64) -> i64 {
        self.stock(ean, 1).await
    }
    pub async fn bodega(&self, ean: i64) -> i64 {
        self.stock(ean, 2).await
    }
    pub async fn apartados(&self, ean: i64) -> i64 {
        self.escalar_i64("SELECT COALESCE(SUM(cantidad), 0) AS n FROM apartados WHERE codigo_ean = ?", ean)
            .await
    }

    /// ¿La bitácora tiene un evento cuyo detalle contiene `texto`?
    pub async fn en_bitacora(&self, texto: &str) -> bool {
        sqlx::query("SELECT 1 FROM bitacora WHERE detalle LIKE ? LIMIT 1")
            .bind(format!("%{texto}%"))
            .fetch_optional(&self.pool)
            .await
            .unwrap()
            .is_some()
    }

    /// Devolución a proveedor PENDIENTE hecha por `vendedor` (token) con una
    /// partida del producto (motivo 1). Devuelve su id.
    pub async fn devolucion(&self, vendedor: &str, p: &Producto, cantidad: i32) -> i64 {
        let r = self
            .post(vendedor, "/api/vendedor/devoluciones", json!({
                "id_proveedor": p.proveedor, "tipo_producto": p.tipo_json(),
                "items": [{ "codigo_ean": p.ean, "cantidad": cantidad, "id_motivo": 1 }]
            }))
            .await
            .exito();
        r.data()["id_devolucion"].as_i64().expect("la devolución no trae id")
    }

    /// Aprueba (o rechaza con nota) una devolución como Jefe.
    pub async fn evaluar(&self, jefe: &str, id: i64, aprobar: bool, notas: &str) -> Resp {
        self.put(jefe, &format!("/api/jefe/devoluciones/{id}/aprobar"), json!({ "aprobar": aprobar, "notas": notas }))
            .await
    }

    /// Crea un usuario con el rol dado (como Gerente) y devuelve (id, contraseña).
    pub async fn usuario_nuevo(&self, rol: i32) -> (i64, &'static str) {
        let g = self.token(GERENTE).await;
        let id = 72_000_000 + unico();
        self.post(&g, "/api/usuarios", json!({
            "id_usuarios": id, "id_roles": rol, "nombre": "Usuario", "apellido_paterno": "Prueba", "contrasena": "clave123"
        }))
        .await
        .exito();
        (id, "clave123")
    }

    // ----- Productos de prueba -----

    /// Libro nuevo del proveedor dado, con stock en los muebles de pruebas.
    pub async fn libro(&self, proveedor: i32, precio: f64, piso: i32, bodega: i32) -> Producto {
        let p = Producto::nuevo(TipoPrueba::Libro, proveedor, precio);
        p.insertar(self, piso, bodega).await;
        p
    }

    /// Revista nueva (editorial 1, Planeta Mexico Infantil) con stock.
    pub async fn revista(&self, proveedor: i32, precio: f64, piso: i32, bodega: i32) -> Producto {
        let p = Producto::nuevo(TipoPrueba::Revista, proveedor, precio);
        p.insertar(self, piso, bodega).await;
        p
    }

    pub async fn poner_stock(&self, ean: i64, mueble: i32, cantidad: i32) {
        sqlx::query(
            "INSERT INTO existencias (codigo_ean, id_mueble, cantidad) VALUES (?, ?, ?) \
             ON DUPLICATE KEY UPDATE cantidad = VALUES(cantidad)",
        )
        .bind(ean)
        .bind(mueble)
        .bind(cantidad)
        .execute(&self.pool)
        .await
        .unwrap();
    }
}

// ---------------------------------------------------------------------------
// Datos de prueba únicos
// ---------------------------------------------------------------------------

static CONTADOR: AtomicI64 = AtomicI64::new(1);

/// Número único dentro de la corrida (la base se recrea en cada corrida).
pub fn unico() -> i64 {
    CONTADOR.fetch_add(1, Ordering::SeqCst)
}

/// Texto único y corto para catálogos ("Prueba 17").
pub fn nombre(base: &str) -> String {
    format!("{base} {}", unico())
}

/// Texto único SOLO con letras (géneros, motivos y autores rechazan números):
/// "Prueba Bcdf".
pub fn nombre_letras(base: &str) -> String {
    const LETRAS: &[u8] = b"bcdfghjklmnpqrstvwxyz";
    let mut n = unico() + 1000;
    let mut sufijo = String::new();
    while n > 0 {
        sufijo.push(LETRAS[(n % LETRAS.len() as i64) as usize] as char);
        n /= LETRAS.len() as i64;
    }
    let mut s = sufijo.chars();
    let primera = s.next().unwrap().to_ascii_uppercase();
    format!("{base} {primera}{}", s.collect::<String>())
}

/// Dígito verificador EAN-13 de los primeros 12 dígitos.
pub fn digito_verificador(doce: i64) -> i64 {
    let s = format!("{doce:012}");
    let suma: u32 = s
        .chars()
        .enumerate()
        .map(|(i, c)| c.to_digit(10).unwrap() * if i % 2 == 0 { 1 } else { 3 })
        .sum();
    ((10 - suma % 10) % 10) as i64
}

/// EAN-13 válido y único: `prefijo` (978/979 libros, 977 revistas) + "9" +
/// 8 dígitos del contador + dígito verificador. La semilla no usa 9789/9779.
pub fn ean_unico(prefijo: i64) -> i64 {
    let doce = prefijo * 1_000_000_000 + 900_000_000 + unico();
    doce * 10 + digito_verificador(doce)
}

/// SKU único de 7 dígitos (la semilla no pasa de 9777668).
pub fn sku_unico() -> i64 {
    9_800_000 + unico()
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TipoPrueba {
    Libro,
    Revista,
}

#[derive(Clone, Debug)]
pub struct Producto {
    pub ean: i64,
    pub sku: i64,
    pub titulo: String,
    pub precio: f64,
    pub proveedor: i32,
    pub tipo: TipoPrueba,
}

impl Producto {
    fn nuevo(tipo: TipoPrueba, proveedor: i32, precio: f64) -> Self {
        let ean = ean_unico(if tipo == TipoPrueba::Libro { 978 } else { 977 });
        let titulo = match tipo {
            TipoPrueba::Libro => format!("Libro de prueba {ean}"),
            TipoPrueba::Revista => format!("Revista de prueba {ean}"),
        };
        Producto { ean, sku: sku_unico(), titulo, precio, proveedor, tipo }
    }

    async fn insertar(&self, c: &Ctx, piso: i32, bodega: i32) {
        let tipo = if self.tipo == TipoPrueba::Libro { "Libro" } else { "Revista" };
        sqlx::query("INSERT INTO productos (codigo_ean, id_proveedor, tipo_producto, SKU, precio) VALUES (?, ?, ?, ?, ?)")
            .bind(self.ean)
            .bind(self.proveedor)
            .bind(tipo)
            .bind(self.sku)
            .bind(self.precio)
            .execute(&c.pool)
            .await
            .unwrap();
        match self.tipo {
            TipoPrueba::Libro => {
                sqlx::query("INSERT INTO libros (codigo_ean, nombre_libro) VALUES (?, ?)")
                    .bind(self.ean)
                    .bind(&self.titulo)
                    .execute(&c.pool)
                    .await
                    .unwrap();
                // Autor 2 de la semilla (Gabriel García Márquez): aparece en el acta.
                sqlx::query("INSERT INTO autores_has_libros (codigo_ean, id_autores) VALUES (?, 2)")
                    .bind(self.ean)
                    .execute(&c.pool)
                    .await
                    .unwrap();
            }
            TipoPrueba::Revista => {
                sqlx::query(
                    "INSERT INTO revistas (codigo_ean, id_editorial, nombre_revista, numero_edicion, periodicidad) \
                     VALUES (?, 1, ?, 12, 'Mensual')",
                )
                .bind(self.ean)
                .bind(&self.titulo)
                .execute(&c.pool)
                .await
                .unwrap();
            }
        }
        sqlx::query("INSERT INTO productos_has_genero (codigo_ean, id_genero) VALUES (?, ?)")
            .bind(self.ean)
            .bind(GENERO_PRUEBAS)
            .execute(&c.pool)
            .await
            .unwrap();
        if piso > 0 {
            c.poner_stock(self.ean, MUEBLE_PISO, piso).await;
        }
        if bodega > 0 {
            c.poner_stock(self.ean, MUEBLE_BODEGA, bodega).await;
        }
    }

    /// Segmento de ruta para traslados y PDF: "libros" o "revistas".
    pub fn ruta(&self) -> &'static str {
        if self.tipo == TipoPrueba::Libro { "libros" } else { "revistas" }
    }
    pub fn tipo_json(&self) -> &'static str {
        if self.tipo == TipoPrueba::Libro { "libro" } else { "revista" }
    }
}

#[cfg(test)]
mod tests_infraestructura {
    use super::{con_base, digito_verificador, ean_unico};

    #[test]
    fn cambia_el_nombre_de_la_base() {
        assert_eq!(con_base("mysql://root@127.0.0.1:3306/Libreria", "X"), "mysql://root@127.0.0.1:3306/X");
        assert_eq!(con_base("mysql://u:p@h/Libreria?ssl=0", "X"), "mysql://u:p@h/X");
        assert_eq!(con_base("mysql://u@h:3306", "X"), "mysql://u@h:3306/X");
    }

    #[test]
    fn genera_ean_con_digito_verificador_correcto() {
        // EAN reales de la semilla.
        assert_eq!(digito_verificador(978607383201), 4);
        assert_eq!(digito_verificador(977123456701), 0);
        let e = ean_unico(978);
        assert_eq!(e.to_string().len(), 13);
        assert!(crate::productos::model::validar_patron_ean13(e).is_ok());
    }
}
