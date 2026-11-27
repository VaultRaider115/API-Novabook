# CHANGELOG — API REST NovaBook

Todos los cambios relevantes del backend se registran aquí siguiendo Keep a Changelog.

## [Unreleased]

Rama `integracion-validaciones`: migración al esquema unificado
(`libreria_unificada-definitivo.sql`) y reglas del diagrama de procesos.

### Added
- Pruebas unitarias por caso de uso (21-nov, rama `pruebas-unitarias`):
  - `src/pruebas/`: 217 pruebas en 32 casos de uso (los 19 del cronograma y
    13 agregados del diagrama v4: RF02, RF16, RF22–RF30, registrar devolución
    a proveedor y tickets de venta). Llaman al router real contra la base
    `Libreria_pruebas`, que se crea sola en cada corrida.
  - `docs/pruebas/`: `generar.ps1` + `generar_documento.py` arman el documento
    "Pruebas unitarias NovaBook" (.docx con python-docx y .pdf con Edge) desde
    una corrida real; `resultados_cargo_test.txt` guarda la última corrida.
  - Dependencia de desarrollo: `tower` (`util`) para llamar al router sin red.
  - `pruebas_unitarias/`: `Ejecutar_pruebas_unitarias.bat` (menú de 4
    opciones: revisión e hito según el cronograma antes de los cambios y según
    el actualizado) y `Pruebas_unitarias_NovaBook.ps1`, que corre las pruebas
    y deja un reporte (.txt, .html, .csv y log) en `pruebas_unitarias/Reportes/`
    (ignorado por git); más la guía de pruebas de la app y un LEEME con los
    requisitos de cada laptop.
  - Resultado: 214 aprobadas y 3 fallidas que exhiben dos defectos abiertos
    (D-01 tipo del acta PDF; D-02 bloqueo mutuo en requisiciones simultáneas).
- Cambios del hito 2 (21-nov, rama `hito2-cambios`):
  - **No se vende más barato de lo que se compra**: la compra rechaza (400)
    una partida cuyo costo unitario pase del precio de venta del producto
    ("… es mayor que su precio de venta …: se vendería con pérdida"); costo
    igual al precio sí se permite (`costo_supera_precio`, con prueba).
  - `GET /api/jefe/ventas?desde&hasta&q`: tickets de venta para el Jefe
    (por omisión, los de hoy), con vendedor, cliente, pago, total, títulos,
    piezas y piezas ya devueltas; `q` busca por número de ticket (`#12`),
    cliente o vendedor (hasta 60 caracteres).
  - `GET /api/jefe/tickets/{id}`: detalle del ticket (lo vendido y lo
    devuelto), el mismo de la devolución de cliente.
  - `test_e2e.sh`: sección 12 con estos cambios (12/12).
- Revisión del 19-nov (3a):
  - Búsqueda de productos (`validar_busqueda_producto`): hasta 30
    caracteres; si es un EAN (solo dígitos con prefijo 977/978/979), hasta
    13 dígitos ("Un EAN tiene 13 dígitos"). Con pruebas.
  - `src/limites.rs` reúne todos los límites (motivos, muebles, catálogos,
    proveedor, título, factura, observaciones, usuarios) y `validar_largo_opt`
    y `validar_monto`; los servicios ya no repiten números sueltos y todos
    responden "… no puede pasar de N caracteres (tiene M)". Se agregó el
    largo en proveedores, autores, editoriales, título y periodicidad.
  - La compra exige costo unitario mayor a $0 (`validar_monto`); el precio
    del alta usa la misma regla (tope $99,999.99).
  - `GET /api/vendedor/devoluciones/validar?id_proveedor&codigo_ean&cantidad`:
    valida una partida al añadirla (existe, es de la editorial elegida y
    alcanza bodega + apartados), con nombres en el mensaje.
  - Ventas y devoluciones de cliente solo aceptan EFECTIVO o TARJETA
    (`METODOS_PAGO`); las de antes con TRANSFERENCIA se conservan (sin
    migración: `metodo_pago` es VARCHAR).
  - Pruebas: `validar_largo_opt`, `validar_monto` y transferencia rechazada;
    `test_e2e.sh` revisa costo $0, EAN fuera del catálogo, transferencia y
    la validación de editorial.
- Revisión del 19-nov:
  - `GET /api/vendedor/inventarios/{id}/devolucion`: lo que va a devolución
    mientras se cuenta, con total a devolver por título, encontradas y
    faltantes, y el contador general. Pruebas de `armar_pendientes`.
  - `src/limites.rs`: límites de texto como constantes (cliente 60, búsqueda
    100) y `validar_largo` con mensaje claro; se valida en la venta y en la
    búsqueda de existencias. Pruebas de `validar_largo`; `test_e2e.sh` revisa
    el cliente de 61 caracteres y el conteo de devolución.
- Revisión del equipo (16-nov):
  - `GET /api/jefe/existencias`: título, stock y `id_proveedor` de cada EAN
    al capturar una solicitud de editorial (existencias trae `id_proveedor`
    para todos los roles).
  - Las solicitudes de editorial traen `piezas` por título (piso, bodega y
    apartados: lo que habría que regresar).
  - `GET /api/vendedor/inventarios/actual` trae el `motivo_devolucion`
    mientras el inventario está abierto (misma regla del cierre), para que el
    Vendedor separe esos títulos al contar.
  - Muebles: el código debe empezar con `P-` (Piso de Venta) o `B-` (Bodega)
    según el área, seguido del número.
  - Alta de libros y revistas: el precio de venta debe ser mayor a $0.
- El alta de libros y revistas rechaza un EAN ya registrado (antes
  sobrescribía título, precio, SKU y autores); las piezas entran por Compras.
- **Compra incompleta** (diagrama, paso 1):
  - Cada partida acepta `cantidad_facturada`; si llegó menos, se abre una
    incidencia y la respuesta trae el borrador del correo al proveedor.
  - `GET /api/bodega/incidencias`, `GET /api/bodega/incidencias/{id}` y
    `POST /api/bodega/incidencias/{id}/reposicion` (lo que llega después;
    cierra la incidencia al completarse).
- **Recepción de la salida en piso** (pasos 2 y 3):
  - Las requisiciones quedan en las tablas `requisicion` y
    `detalle_requisicion`.
  - `GET /api/bodega|vendedor/requisiciones` y `.../{folio}`.
  - `POST /api/vendedor/requisiciones/{folio}/recepcion`: confirma o corrige
    lo recibido (ajusta stock y movimientos del folio) y registra el mueble
    de piso de cada libro.
  - Migración: `migracion_recepcion_incidencias.sql`.
- La API avisa al arrancar si falta cualquiera de las migraciones.
- e2e: sección 11 (incidencia y recepción).
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
- El error de capacidad de un mueble dice cuántas piezas tiene: "La
  capacidad máxima (X) no puede ser menor a las N piezas que ya tiene el
  mueble".
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
- **Documentación**:
  - `AI_CONTEXT.md` reescrito con el estado real del backend: rutas por rol,
    reglas, migraciones, pruebas y despliegue.
  - `README.md` nuevo: guía de arranque (requisitos, base de datos y
    migraciones, `.env` de ejemplo, ejecución, usuarios de prueba, pruebas
    y despliegue).
  - `CHANGELOG.md` con los cambios de esta rama.

### Fixed
- Los mensajes de stock dicen "Piso de Venta" (antes "Piso de Ventas").
- Fechas en hora local: sqlx fijaba la sesión en UTC y, después de las
  18:00, los eventos quedaban con la fecha del día siguiente (el Historial
  de Movimientos de hoy salía vacío). La conexión usa la zona del servidor
  de BD o la de `DB_TIME_ZONE` si se define.
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
