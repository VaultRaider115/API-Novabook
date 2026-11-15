# CHANGELOG — API REST NovaBook

Todos los cambios relevantes del backend se registran aquí siguiendo Keep a Changelog.

## [Unreleased]

Rama `integracion-validaciones`: migración al esquema unificado
(`libreria_unificada-definitivo.sql`) y reglas del diagrama de procesos.

### Added
- **Stock por mueble**: alta, compras, traslados y ventas usan `existencias`
  por mueble y `movimientos` con mueble de origen y destino, en
  transacciones.
- **CRUD de `/api/editoriales`**.
- **Requisiciones con folio** `REQ-AAAA-n`:
  - `POST /api/bodega/requisiciones` (salida a piso) y
    `POST /api/vendedor/requisiciones` (entrada a bodega).
  - Varias partidas en un documento, todo o nada.
- **Devolución de cliente con ticket**:
  - `GET /api/vendedor/tickets/{id}` y `POST /api/vendedor/devoluciones-cliente`.
  - Reembolso o cambio por igual o mayor valor; las piezas con defecto van
    a la nueva tabla `apartados`.
  - Migración: `migracion_devolucion_cliente.sql`.
- **Inventario mensual**:
  - Vendedor: `/api/vendedor/inventarios` (abrir, contar, cerrar).
  - Jefe: `/api/jefe/inventarios` (revisar).
  - Solicitudes de editorial en `/api/jefe/solicitudes-editorial`.
  - Se marca a devolución lo añejado (90 días sin movimiento) y lo que pidió
    la editorial.
  - Migración: `migracion_inventario_mensual.sql`.
- **Devoluciones (consulta)**:
  - El Vendedor consulta las suyas (`GET /api/vendedor/devoluciones`).
  - El Gerente consulta el historial y descarga actas en solo lectura
    (`/api/gerente/devoluciones/...`).
- **Existencias para Bodega**: `GET /api/bodega/existencias`, de solo
  lectura, para armar la salida con título y editorial.
- **Columna `devolucion.observaciones`** para la nota del Jefe al aprobar o
  rechazar. Migración `migracion_observaciones_devolucion.sql` (copia las
  notas de la bitácora). La API avisa al arrancar si falta.
- **Bitácora**: `GET /api/jefe/bitacora` con `desde`, `hasta`, `categoria`,
  `page` y `per_page`. Las categorías son VENTA, TRASLADO, COMPRA, ACCESO,
  DEVOLUCIÓN, INVENTARIO, USUARIOS, CATÁLOGO y SISTEMA, con descripciones
  legibles ("Venta completada Ticket #…").
- **Acta PDF de devolución**:
  - Igual al diseño 21: autor o edición bajo el título y EAN en Courier.
  - Columnas COSTO (de la última compra) y P. VENTA, con sus importes.
  - Los renglones de la devolución traen `descripcion`, `costo_unitario` y
    `precio_venta`.
- **Pruebas**:
  - `test_e2e.sh` con 10 secciones de punta a punta.
  - Pruebas unitarias de PDF, dinero, anchos de texto y retorno máximo a
    bodega.

### Changed
- **Escritura de catálogos** según el diagrama:
  - Autores, géneros, motivos y muebles: solo el Jefe.
  - Editoriales y proveedores: CRUD del Jefe; Bodega solo da de alta.
  - El Gerente no administra catálogos.
- **Reglas del diagrama de procesos**:
  - Una sola editorial por requisición, por compra y por devolución.
  - De piso a bodega solo regresa hasta la mitad de la última bajada (sin
    límite si nunca se bajó).
- **Devolución a la editorial**:
  - Límite de 60 piezas; solo productos del proveedor.
  - Descuenta primero apartados y luego bodega.
  - El rechazo exige observaciones.
- **Usuarios**:
  - Solo el Gerente crea usuarios y asigna roles; el Jefe edita y da de
    baja, sin tocar roles ni cuentas de Gerente.
  - Nadie cambia su propio rol ni deja el sistema sin Gerente activo.
  - Teléfono solo con dígitos y sin 0 inicial.
- **SKU** de 6 a 7 dígitos, obligatorio en el alta y único. EAN-13 con dígito
  verificador (libros 978/979, revistas 977).
- **Bitácora**: se quitó el middleware que registraba cada petición GET/POST;
  ahora solo hay eventos de negocio y de acceso.
- **Documentación**: `AI_CONTEXT.md` reescrito con el estado real del
  backend.

### Fixed
- Devoluciones sin importar mayúsculas o minúsculas en el tipo de producto.
- Borrar un autor con libros asociados ya no rompe la integridad.
- Semilla: el EAN de "Muy Interesante" pasa a 9772007490009, con el dígito
  verificador correcto.

### Security
- Búsqueda de existencias sin inyección SQL (consultas con parámetros).
- Login solo con contraseñas bcrypt; se quitó el respaldo en texto plano.
- Un usuario dado de baja pierde el acceso de inmediato: el middleware
  revisa `usuarios.estado` en cada petición.
- El acta PDF solo existe si la devolución está aprobada y solo la descargan
  su solicitante, el Jefe o el Gerente.
- El log de arranque oculta la contraseña de `DATABASE_URL`.

## [0.1.0] - 2026-09-18

### Added
- Backend en Rust con Axum y SQLx.
- Módulos de usuarios y productos.
- Autenticación con JWT y bcrypt.
- Capa de permisos por roles.
- CRUD de usuarios y flujo de negocio para catálogo, inventario, compras, ventas y devoluciones.
- Generación de PDF para devoluciones.

### Changed
- Ajuste modular para separar rutas de catálogo, bodega, vendedor y jefe.
- Fortalecimiento del manejo de errores y auditoría.
