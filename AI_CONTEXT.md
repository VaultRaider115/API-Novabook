# Contexto técnico del backend NovaBook

Estado actual de la API REST de NovaBook (ERP de la librería Sanborns
Saltillo), basado en el código de `src/`. Si algo de aquí no coincide con el
código, manda el código.

## 1. Stack

- Rust 2024, Tokio, Axum 0.8
- SQLx 0.9 con MySQL/MariaDB (esquema unificado, BD `Libreria`)
- JWT (`jsonwebtoken`) y bcrypt
- dotenvy, tower-http (CORS)
- PDF generado a mano, sin dependencias (`src/services/pdf.rs`)

## 2. Arranque (`src/main.rs`)

1. `Config::from_env()` lee la configuración.
2. `create_pool()` abre el pool; el log muestra la URL con la contraseña
   oculta (`db::url_sin_contrasena`).
3. `db::verificar_esquema()` avisa si falta una migración (hoy revisa la
   columna `devolucion.observaciones`).
4. Se montan los routers de `usuarios` y `productos` con CORS abierto.
5. No hay middleware de auditoría por petición: la bitácora solo registra
   eventos de negocio y de acceso, que escribe cada servicio.

## 3. Configuración (`src/config.rs`)

| Variable | Obligatoria | Default |
|---|---|---|
| `DATABASE_URL` | sí | — |
| `JWT_SECRET` | sí | — |
| `JWT_EXPIRATION_HOURS` | no | 24 |
| `SERVER_HOST` | no | `0.0.0.0` |
| `SERVER_PORT` | no | 8080 |
| `DB_TIME_ZONE` | no | zona del servidor de BD (ej. `-06:00`) |

En local se leen de `.env` (no se sube al repositorio).

## 4. Arquitectura

Capas por dominio: **controller → service → repo → model**.

- `usuarios/`: login, perfil y CRUD de usuarios.
- `productos/`: catálogos, alta, stock, movimientos, ventas, compras,
  devoluciones, inventario mensual y bitácora.
- `middleware/auth.rs`: JWT y guardas por rol.
- `services/pdf.rs`: acta de devolución en PDF.
- `db.rs`: pool, `AppState`, helpers `row_get_*` y `verificar_esquema`.
- `error.rs`: `AppError` → `BadRequest` 400, `Unauthorized` 401,
  `Forbidden` 403, `NotFound` 404, `Conflict` 409, `Internal`/`Database` 500.

Respuestas JSON: `{"success": true, "data": …}` o
`{"success": false, "error": {"code", "message"}}`.

SQLx rechaza SQL dinámico sin auditar: las consultas son `&'static str`,
`QueryBuilder` con binds, o `AssertSqlSafe` sobre `format!` de constantes.

### Stock

- `existencias` guarda el stock por mueble; cada mueble pertenece a una
  ubicación (`UBICACION_TIENDA = 1` piso de venta, `UBICACION_BODEGA = 2`).
- `apartados` guarda las piezas con defecto que devolvió un cliente (no se
  venden).
- `productos/repo/stock.rs` concentra las operaciones transaccionales
  (`Tx<'a>`): bloquear, descontar, sumar, elegir mueble y lo mismo para
  apartados.
- `movimientos` registra cada traslado con `mueble_origen`/`mueble_destino`;
  un traslado puede generar varias filas (una por mueble de origen) con la
  misma `fecha_hora`.

## 5. Roles y seguridad

| id | Rol |
|---|---|
| 1 | Jefe de Departamento |
| 2 | Personal de Bodega |
| 3 | Vendedor |
| 4 | Gerente |

- `auth_middleware` valida el JWT y en **cada petición** revisa
  `usuarios.estado`: un usuario dado de baja recibe 401 de inmediato.
- Guardas: `require_jefe`, `require_gerente`, `require_bodega`,
  `require_vendedor`, `require_gerente_o_jefe`, `require_jefe_o_bodega`.
- El login solo acepta contraseñas bcrypt (no hay respaldo en texto plano).
- Solo el Gerente crea usuarios y asigna roles. El Jefe edita datos y da de
  baja, pero no toca roles ni cuentas de Gerente. Nadie cambia su propio
  rol ni deja el sistema sin un Gerente activo. Teléfono: solo dígitos y sin
  0 inicial; los nombres se recortan (trim).

## 6. Rutas

### Autenticación y usuarios

- `POST /api/auth/login` (pública)
- `GET /api/auth/perfil`
- `GET|POST /api/usuarios/`, `GET|PUT|DELETE /api/usuarios/{id}`
  (Gerente o Jefe; las restricciones finas están en el servicio)

### Catálogos (`/api`, autenticado)

Lectura para cualquier rol autenticado.

| Ruta | Escritura |
|---|---|
| `/generos`, `/generos/{id}` | Jefe |
| `/autores`, `/autores/{id}` | Jefe |
| `/motivos`, `/motivos/{id}` | Jefe |
| `/muebles`, `/muebles/{id}` | Jefe |
| `/editoriales` (POST) | Jefe o Bodega (alta al recibir la compra) |
| `/editoriales/{id}` (PUT/DELETE) | Jefe |
| `/proveedores` (POST) | Jefe o Bodega (la app ya no muestra el alta a Bodega: decisión del equipo, 19-nov) |
| `/proveedores/{id}` (PUT/DELETE) | Jefe |

El Gerente no administra catálogos. No se permiten registros repetidos.

### Bodega (`/api/bodega`)

- `POST /libros`, `POST /revistas`: alta de producto.
- `POST /movimientos/libros`, `POST /movimientos/revistas`: traslado suelto
  bodega → piso.
- `POST /requisiciones`: requisición de salida bodega → piso.
- `GET /requisiciones?tipo&estado`, `GET /requisiciones/{folio}`: estado de
  las salidas (seguimiento y reimpresión).
- `GET /existencias`: títulos y editorial para armar la salida.
- `POST /compras`, `GET /compras`: compra a la editorial. Cada partida lleva
  `cantidad` (lo que llegó) y `cantidad_facturada` (opcional); si llegó menos,
  la respuesta trae `incidencia`.
- `GET /incidencias?estado`, `GET /incidencias/{id}`,
  `POST /incidencias/{id}/reposicion`: compras incompletas.

### Vendedor (`/api/vendedor`)

- `POST /ventas`, `GET /existencias`
- `POST /movimientos/libros`, `POST /movimientos/revistas`: traslado suelto
  piso → bodega.
- `POST /requisiciones`: requisición de entrada piso → bodega.
- `GET /requisiciones?tipo&estado`, `GET /requisiciones/{folio}`,
  `POST /requisiciones/{folio}/recepcion`: recibir en piso una salida.
- `GET /tickets/{id}`, `POST /devoluciones-cliente`: devolución de cliente.
- `POST /devoluciones`, `GET /devoluciones` (solo las suyas): devolución a
  la editorial.
- `GET /devoluciones/{id}/pdf/libros|revistas`: acta.
- `POST /inventarios`, `GET /inventarios/actual`,
  `PUT /inventarios/{id}/conteos`, `POST /inventarios/{id}/cerrar`.

### Jefe (`/api/jefe`)

- `GET /devoluciones/historial`, `PUT /devoluciones/{id}/aprobar`,
  `GET /devoluciones/{id}/pdf/libros|revistas`
- `GET /movimientos/diarios`
- `GET /bitacora?desde&hasta&categoria&page&per_page`
- `GET /inventarios`, `GET /inventarios/{id}`, `PUT /inventarios/{id}/revisar`
- `GET|POST /solicitudes-editorial`, `PUT /solicitudes-editorial/{id}/atender`
  (cada título trae `piezas`: piso + bodega + apartados)
- `GET /existencias`: título, stock e `id_proveedor` al capturar la solicitud.

### Gerente (`/api/gerente`, solo lectura de devoluciones)

- `GET /devoluciones/historial`
- `GET /devoluciones/{id}/pdf/libros|revistas`

## 7. Reglas de negocio implementadas

- **EAN-13** con dígito verificador: libros 978/979, revistas 977. **SKU** de
  6 a 7 dígitos (100000–9999999), obligatorio en el alta y sin repetirse.
  El alta es solo para EAN nuevos: si el EAN ya existe responde 409 ("ya
  está registrado como …; para agregar piezas usa Compras a Proveedor").
- **Compra**: una sola editorial (proveedor) por factura; costo no
  negativo, factura de hasta 30 caracteres; las piezas entran a un mueble de
  bodega. `detalles_compras` guarda solo el subtotal: el costo unitario es
  `subtotal / cantidad`.
- **Paquete incompleto** (paso 1, `IncidenciaRepo`): `cantidad` es lo que
  llegó (puede ser 0) y `cantidad_facturada` lo que dice la factura (por
  omisión, igual). Llegado ≤ facturado y al menos una pieza en total. Si
  falta algo:
  - Se abre una incidencia ABIERTA (`incidencia_compra`, una por compra) con
    los faltantes; solo entra a bodega lo recibido.
  - La API arma el correo al proveedor (`correo`: para, asunto, cuerpo, con
    `proveedor.correo` y la lista de lo pendiente); el cliente lo abre en el
    programa de correo del equipo (mailto:). No hay SMTP.
  - `POST /incidencias/{id}/reposicion`: lo que llega después entra a bodega,
    se suma a la compra original (`detalles_compras` y total, al costo de la
    factura) y la incidencia se CIERRA cuando ya no falta nada. No se acepta
    más de lo pendiente.
- **Requisición** (`MovimientoRepo::registrar_requisicion`): un documento con
  folio `REQ-AAAA-n` (año + id del primer movimiento), todo o nada; el folio
  queda en `movimientos.observaciones`. Reglas:
  - Una sola editorial por documento (`productos.id_proveedor`).
  - Piso → bodega: solo hasta la mitad (redondeo hacia abajo) de la última
    bajada bodega → piso del EAN, contando lo ya regresado desde entonces;
    sin límite si nunca se bajó. También aplica a los traslados sueltos
    (`trasladar_en_tx`).
  - Cada requisición queda en `requisicion` / `detalle_requisicion` (con el
    mueble destino de cada EAN). La de entrada queda REGISTRADA; la de
    salida, PENDIENTE hasta que el Vendedor la recibe.
- **Recepción de la salida** (pasos 2 y 3, `RecepcionRepo`): el Vendedor
  captura lo que llegó de cada partida (todas, una vez cada una) y el mueble
  de piso donde quedó. Solo una vez por folio (después, 409).
  - Todo coincide → CONFIRMADA. Si no → CORREGIDA y el cliente la reimprime.
  - Llegaron menos: lo que falta regresa a su mueble de bodega y las filas de
    `movimientos` del folio bajan a lo real. Llegaron más: el sobrante sale
    de bodega con la fecha y nota del folio. Así la regla de la mitad mide la
    bajada real.
  - Cambio de mueble: las piezas pasan al mueble elegido (debe ser de piso y
    tener espacio) y `mueble_destino` del folio se actualiza.
- **Venta**: lleva `nombre_cliente` y `metodo_pago` (no hay tabla de
  clientes); sin stock en piso no se vende.
- **Devolución de cliente**: ticket de 15 días o menos; no más de lo vendido
  (contando devoluciones previas); reembolso al precio del ticket. Sin
  defecto regresa a piso; con defecto va a `apartados`. Cambio solo por igual
  o mayor valor, pagando la diferencia con una venta nueva ligada.
- **Devolución a la editorial**: máximo 60 piezas; solo productos de ese
  proveedor; descuenta primero apartados y luego bodega. Estado
  `PENDIENTE` → `APROBADO`/`RECHAZADO` (solo el Jefe). El rechazo exige
  observaciones (`devolucion.observaciones`, también van a la bitácora).
- **Acta PDF** (diseño 21): solo si está aprobada; la descargan su
  solicitante, el Jefe o el Gerente. Lleva:
  - Bajo el título, el autor del libro o "Edición mensual · No. N" de la
    revista.
  - El EAN en Courier.
  - Columnas COSTO (última compra; N/D si nunca se compró) y P. VENTA
    (`productos.precio`), con sus importes bajo el total.
  - Un pie con quién solicitó y quién autorizó.
- **Inventario mensual**:
  - El Vendedor abre (uno a la vez), registra conteos y cierra.
  - Al cerrar se guarda el piso del sistema y se marca para devolución lo que
    pidió una editorial (solicitud ABIERTA) o lo añejado (sin ventas ni
    compras en `DIAS_ANEJAMIENTO = 90` días).
  - El Jefe aprueba (ajusta el piso con la diferencia, sin pasar del stock
    actual) o rechaza con motivo.
  - Mientras está ABIERTO, el detalle calcula `motivo_devolucion` con la
    misma regla (añejado solo si se contó algo), para que el Vendedor separe
    esos títulos; al cerrar se guarda.
- **Muebles**: código `P-…` en Piso de Venta y `B-…` en Bodega (máx. 10
  caracteres); la capacidad no baja de las piezas que ya tiene; con piezas no
  cambia de área ni de género.
- **Alta de producto**: precio de venta mayor a $0.

## 8. Bitácora

- `GET /api/jefe/bitacora` pagina en el servidor y filtra por categoría.
- La categoría se calcula en SQL (`CATEGORIA_SQL`): VENTA, TRASLADO,
  COMPRA, ACCESO, DEVOLUCIÓN, INVENTARIO, USUARIOS, CATÁLOGO, SISTEMA.
- La descripción es una frase legible que escribe cada servicio; por
  ejemplo, "Requisición REQ-2026-1 Bodega-Piso (Planeta Mexico): 3 unidades
  en 2 partida(s)".

## 9. Base de datos y scripts

- `libreria_unificada-definitivo.sql`: esquema y semilla (copia idéntica de
  `../docs/libreria_unificada-definitivo.sql`); reimportarlo borra la base.
- `seeds.sql`: datos de prueba.
- Migraciones idempotentes para bases creadas antes de cada cambio:
  - `migracion_observaciones_devolucion.sql`: columna `observaciones`; copia
    las notas que había en la bitácora.
  - `migracion_devolucion_cliente.sql`: devoluciones de cliente y `apartados`.
  - `migracion_inventario_mensual.sql`: inventario, detalle y solicitudes de
    editorial.
  - `migracion_recepcion_incidencias.sql`: `incidencia_compra`,
    `requisicion` y sus detalles.
- La API avisa al arrancar si falta alguna de las cuatro migraciones
  (`db::verificar_esquema`).
- `migrations_v2.sql`, `backup_libreria.sql`: históricos.

Usuarios de prueba: `1001/admin123` (Gerente), `350976899/2501` (Jefe),
`628777130/7777` (Bodega), `628777129/8888` (Vendedor).

## 10. Pruebas

- `cargo test`: pruebas unitarias (PDF, anchos de texto, formato de dinero,
  retorno máximo a bodega, URL sin contraseña, etc.).
- `cargo clippy --all-targets`: debe quedar sin avisos.
- `test_e2e.sh`: 11 secciones de punta a punta contra una base recién
  importada y la API corriendo. En Windows hay que usar el curl de System32,
  porque el de Git Bash rompe los acentos:
  `tr -d '\r' < test_e2e.sh > /tmp/e2e.sh && PATH=/c/WINDOWS/system32:$PATH bash /tmp/e2e.sh`

## 11. Despliegue

- `Dockerfile` (Ubuntu 24.04, multi-etapa, puerto 8080).
- GitHub Actions (`.github/workflows/deploy.yaml`) publica la imagen en GHCR.
- Manifiestos de K3s en `k3s/` (`backend.yaml`, `database.yaml`).
