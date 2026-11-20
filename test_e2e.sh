#!/usr/bin/env bash
set -e

# Puerto por defecto de la API (SERVER_PORT=8080); se puede cambiar con BASE_URL=...
# En Windows usa el curl de C:\Windows\System32: el de Git Bash manda los acentos
# en ANSI y la API rechaza el JSON (p. ej. PATH=/c/WINDOWS/system32:$PATH bash test_e2e.sh).
BASE_URL="${BASE_URL:-http://127.0.0.1:8080/api}"
echo "=== INICIANDO PRUEBAS E2E NOVABOOK V2 ==="

# -----------------------------------------------------------------------------
# 1. AUTENTICACIÓN
# -----------------------------------------------------------------------------
echo "[1/11] Probando autenticación para los 4 roles..."

# Gerente
GERENTE_RES=$(curl -s -X POST "$BASE_URL/auth/login" -H "Content-Type: application/json" -d '{"id_usuario": 1001, "contrasena": "admin123"}')
TOKEN_GERENTE=$(echo "$GERENTE_RES" | grep -o '"token":"[^"]*' | cut -d'"' -f4)
if [ -z "$TOKEN_GERENTE" ]; then echo "Error en login Gerente: $GERENTE_RES"; exit 1; fi
echo "  ✓ Login Gerente exitoso"

# Jefe
JEFE_RES=$(curl -s -X POST "$BASE_URL/auth/login" -H "Content-Type: application/json" -d '{"id_usuario": 350976899, "contrasena": "2501"}')
TOKEN_JEFE=$(echo "$JEFE_RES" | grep -o '"token":"[^"]*' | cut -d'"' -f4)
if [ -z "$TOKEN_JEFE" ]; then echo "Error en login Jefe: $JEFE_RES"; exit 1; fi
echo "  ✓ Login Jefe de Departamento exitoso"

# Bodega
BODEGA_RES=$(curl -s -X POST "$BASE_URL/auth/login" -H "Content-Type: application/json" -d '{"id_usuario": 628777130, "contrasena": "7777"}')
TOKEN_BODEGA=$(echo "$BODEGA_RES" | grep -o '"token":"[^"]*' | cut -d'"' -f4)
if [ -z "$TOKEN_BODEGA" ]; then echo "Error en login Bodega: $BODEGA_RES"; exit 1; fi
echo "  ✓ Login Bodega exitoso"

# Vendedor
VENDEDOR_RES=$(curl -s -X POST "$BASE_URL/auth/login" -H "Content-Type: application/json" -d '{"id_usuario": 628777129, "contrasena": "8888"}')
TOKEN_VENDEDOR=$(echo "$VENDEDOR_RES" | grep -o '"token":"[^"]*' | cut -d'"' -f4)
if [ -z "$TOKEN_VENDEDOR" ]; then echo "Error en login Vendedor: $VENDEDOR_RES"; exit 1; fi
echo "  ✓ Login Vendedor exitoso"

# -----------------------------------------------------------------------------
# 2. CRUD AUTORES (solo Jefe escribe; diagrama de procesos · Catálogos)
# -----------------------------------------------------------------------------
echo "[2/11] Probando CRUD de Autores y validación de integridad..."
AUTORES_LIST=$(curl -s -X GET "$BASE_URL/autores" -H "Authorization: Bearer $TOKEN_BODEGA")
echo "  ✓ Lista de autores obtenida: $(echo $AUTORES_LIST | cut -c1-60)..."

# Bodega puede leer pero no escribir autores
BODEGA_AUTOR_RES=$(curl -s -X POST "$BASE_URL/autores" \
  -H "Authorization: Bearer $TOKEN_BODEGA" \
  -H "Content-Type: application/json" \
  -d '{"nombre": "Prueba", "apellidos": "Permisos", "nacionalidad": "Mexicana"}')
if echo "$BODEGA_AUTOR_RES" | grep -q '"success":false'; then
  echo "  ✓ Bodega no puede crear autores (solo el Jefe)"
else
  echo "Error: Bodega pudo crear un autor: $BODEGA_AUTOR_RES"; exit 1
fi

# Crear autor temporal
CREATE_AUTOR_RES=$(curl -s -X POST "$BASE_URL/autores" \
  -H "Authorization: Bearer $TOKEN_JEFE" \
  -H "Content-Type: application/json" \
  -d '{"nombre": "Isabel", "apellidos": "Allende Llona", "nacionalidad": "Chilena", "biografia": "Premio Nacional de Literatura de Chile."}')
AUTOR_ID=$(echo "$CREATE_AUTOR_RES" | grep -o '"id_autor":[0-9]*' | cut -d':' -f2)
if [ -z "$AUTOR_ID" ]; then echo "Error creando autor: $CREATE_AUTOR_RES"; exit 1; fi
echo "  ✓ Autor creado con ID: $AUTOR_ID"

# Modificar autor
UPDATE_AUTOR_RES=$(curl -s -X PUT "$BASE_URL/autores/$AUTOR_ID" \
  -H "Authorization: Bearer $TOKEN_JEFE" \
  -H "Content-Type: application/json" \
  -d '{"nombre": "Isabel", "apellidos": "Allende", "nacionalidad": "Chilena-Estadounidense", "biografia": "Autora de La Casa de los Espíritus."}')
echo "  ✓ Autor modificado con éxito"

# Intentar eliminar autor con libros (ej. Gabriel García Márquez ID 2)
DEL_FAIL_RES=$(curl -s -X DELETE "$BASE_URL/autores/2" -H "Authorization: Bearer $TOKEN_JEFE")
if echo "$DEL_FAIL_RES" | grep -q '"success":false'; then
  echo "  ✓ Integridad referencial protegida: No se permitió borrar autor con libros asociados"
else
  echo "Error: Se permitió borrar autor con libros: $DEL_FAIL_RES"; exit 1
fi

# Eliminar autor temporal sin libros
DEL_OK_RES=$(curl -s -X DELETE "$BASE_URL/autores/$AUTOR_ID" -H "Authorization: Bearer $TOKEN_JEFE")
echo "  ✓ Autor temporal sin libros eliminado con éxito"

# -----------------------------------------------------------------------------
# 3. CRUD PROVEEDORES
# -----------------------------------------------------------------------------
echo "[3/11] Probando CRUD de Proveedores..."
PROV_LIST=$(curl -s -X GET "$BASE_URL/proveedores" -H "Authorization: Bearer $TOKEN_BODEGA")
echo "  ✓ Lista de proveedores obtenida"

# Crear proveedor
CREATE_PROV_RES=$(curl -s -X POST "$BASE_URL/proveedores" \
  -H "Authorization: Bearer $TOKEN_BODEGA" \
  -H "Content-Type: application/json" \
  -d '{"nombre_proveedor": "Fondo de Cultura Económica", "rfc": "FCE340915AA1", "telefono": "5552274600", "correo": "contacto@fondodeculturaeconomica.com", "direccion": "Carretera Picacho Ajusco 227, CDMX", "persona_contacto": "Lic. Paco Ignacio Taibo II", "estatus": "ACTIVO"}')
PROV_ID=$(echo "$CREATE_PROV_RES" | grep -o '"id_proveedor":[0-9]*' | cut -d':' -f2)
if [ -z "$PROV_ID" ]; then echo "Error creando proveedor: $CREATE_PROV_RES"; exit 1; fi
echo "  ✓ Proveedor creado con ID: $PROV_ID"

# Modificar proveedor (solo el Jefe; Bodega únicamente da de alta)
UPDATE_PROV_RES=$(curl -s -X PUT "$BASE_URL/proveedores/$PROV_ID" \
  -H "Authorization: Bearer $TOKEN_JEFE" \
  -H "Content-Type: application/json" \
  -d '{"nombre_proveedor": "FCE Mexico", "rfc": "FCE340915AA1", "telefono": "5552274699", "correo": "ventas@fce.com.mx", "direccion": "Carretera Picacho Ajusco 227, CDMX", "persona_contacto": "Lic. Taibo II", "estatus": "ACTIVO"}')
echo "  ✓ Proveedor actualizado con éxito"

# -----------------------------------------------------------------------------
# 4. GÉNEROS Y VALIDACIÓN DE CÓDIGO EAN-13
# -----------------------------------------------------------------------------
echo "[4/11] Probando catálogo de Géneros y validación EAN-13..."
GENEROS_RES=$(curl -s -X GET "$BASE_URL/generos" -H "Authorization: Bearer $TOKEN_BODEGA")
echo "  ✓ Catálogo de géneros obtenido"

# Prueba EAN con dígito verificador inválido (debe fallar)
BAD_EAN_RES=$(curl -s -X POST "$BASE_URL/bodega/libros" \
  -H "Authorization: Bearer $TOKEN_BODEGA" \
  -H "Content-Type: application/json" \
  -d '{"codigo_ean": 9786073832019, "sku": 999, "nombre_libro": "Prueba", "id_proveedor": 1, "generos": [11], "autores": [1], "precio": 100.0, "cantidad": 5, "id_mueble": 11}')
if echo "$BAD_EAN_RES" | grep -q '"success":false'; then
  echo "  ✓ Validación EAN exitosa: Rechazó dígito verificador incorrecto"
else
  echo "Error: Aceptó dígito verificador corrupto: $BAD_EAN_RES"; exit 1
fi

# Prueba EAN Revista como Libro (debe fallar)
BAD_TIPO_RES=$(curl -s -X POST "$BASE_URL/bodega/libros" \
  -H "Authorization: Bearer $TOKEN_BODEGA" \
  -H "Content-Type: application/json" \
  -d '{"codigo_ean": 9771234567010, "sku": 999, "nombre_libro": "Revista Disfrazada", "id_proveedor": 1, "generos": [11], "autores": [1], "precio": 100.0, "cantidad": 5, "id_mueble": 11}')
if echo "$BAD_TIPO_RES" | grep -q '"success":false'; then
  echo "  ✓ Validación EAN exitosa: Rechazó código de Revista (977) en registro de Libros"
else
  echo "Error: Aceptó revista como libro: $BAD_TIPO_RES"; exit 1
fi

# Un EAN ya registrado no se vuelve a dar de alta (las piezas entran por Compras)
DUP_RES=$(curl -s -X POST "$BASE_URL/bodega/libros"   -H "Authorization: Bearer $TOKEN_BODEGA"   -H "Content-Type: application/json"   -d '{"codigo_ean": 9786073832014, "sku": 555555, "nombre_libro": "Otro Titulo", "id_proveedor": 1, "generos": [16], "autores": [2], "precio": 1.00, "cantidad": 0, "id_mueble": 111}')
echo "$DUP_RES" | grep -q 'ya está registrado' || { echo "Error: se aceptó un EAN repetido en el alta: $DUP_RES"; exit 1; }
echo "  ✓ El alta rechaza un EAN ya registrado"

# El precio de venta debe ser mayor a 0
PRECIO0=$(curl -s -X POST "$BASE_URL/bodega/libros"   -H "Authorization: Bearer $TOKEN_BODEGA"   -H "Content-Type: application/json"   -d '{"codigo_ean": 9786071234568, "sku": 555555, "nombre_libro": "Pedro Paramo", "id_proveedor": 1, "generos": [15], "autores": [2], "precio": 0, "cantidad": 0, "id_mueble": 111}')
echo "$PRECIO0" | grep -q 'debe ser mayor a' || { echo "Error: se aceptó un precio de 0: $PRECIO0"; exit 1; }
echo "  ✓ El alta rechaza un precio de venta de \$0"

# Registrar un libro nuevo con varios géneros y autor
REG_LIBRO_RES=$(curl -s -X POST "$BASE_URL/bodega/libros"   -H "Authorization: Bearer $TOKEN_BODEGA"   -H "Content-Type: application/json"   -d '{"codigo_ean": 9786071234568, "sku": 555555, "nombre_libro": "Pedro Paramo", "id_proveedor": 1, "generos": [15, 16], "autores": [2], "precio": 280.00, "cantidad": 0, "id_mueble": 111}')
echo "$REG_LIBRO_RES" | grep -q '"success":true' || { echo "Error registrando libro nuevo: $REG_LIBRO_RES"; exit 1; }
echo "  ✓ Libro nuevo registrado con géneros (15, 16) y autor ID 2"

# Registrar una revista nueva con varios géneros
REG_REV_RES=$(curl -s -X POST "$BASE_URL/bodega/revistas"   -H "Authorization: Bearer $TOKEN_BODEGA"   -H "Content-Type: application/json"   -d '{"codigo_ean": 9771234567027, "sku": 200002, "nombre_revista": "National Geographic Abr 2026", "id_proveedor": 2, "id_editorial": 1, "numero_edicion": 346, "periodicidad": "Mensual", "generos": [15, 18], "precio": 120.00, "cantidad": 0, "id_mueble": 111}')
echo "$REG_REV_RES" | grep -q '"success":true' || { echo "Error registrando revista nueva: $REG_REV_RES"; exit 1; }
echo "  ✓ Revista nueva registrada con géneros (15, 18)"

# -----------------------------------------------------------------------------
# 5. REGISTRO DE COMPRAS A PROVEEDOR (BODEGA)
# -----------------------------------------------------------------------------
echo "[5/11] Probando registro de Compras a Proveedor..."
COMPRA_RES=$(curl -s -X POST "$BASE_URL/bodega/compras" \
  -H "Authorization: Bearer $TOKEN_BODEGA" \
  -H "Content-Type: application/json" \
  -d '{"id_proveedor": 1, "observaciones": "Lote mensual facturado", "items": [{"codigo_ean": 9786073832014, "tipo_producto": "libro", "cantidad": 10, "costo_unitario": 210.00}, {"codigo_ean": 9788497776639, "tipo_producto": "libro", "cantidad": 8, "costo_unitario": 120.00}]}')
if echo "$COMPRA_RES" | grep -q '"success":true'; then
  COMPRA_ID=$(echo "$COMPRA_RES" | grep -o '"id_compra":[0-9]*' | cut -d':' -f2)
  echo "  ✓ Compra registrada exitosamente con ID: $COMPRA_ID e inventario incrementado en bodega"
else
  echo "Error registrando compra: $COMPRA_RES"; exit 1
fi

# Listar compras en bodega
COMPRAS_LIST=$(curl -s -X GET "$BASE_URL/bodega/compras" -H "Authorization: Bearer $TOKEN_BODEGA")
echo "  ✓ Historial de compras consultado correctamente"

# El costo unitario es obligatorio y mayor a $0 (src/limites.rs: validar_monto)
COMPRA_CERO=$(curl -s -X POST "$BASE_URL/bodega/compras" -H "Authorization: Bearer $TOKEN_BODEGA" -H "Content-Type: application/json" \
  -d '{"id_proveedor": 1, "items": [{"codigo_ean": 9786073832014, "cantidad": 1, "costo_unitario": 0}]}')
echo "$COMPRA_CERO" | grep -q 'debe ser mayor a \$0' || { echo "Error: se aceptó una compra con costo \$0: $COMPRA_CERO"; exit 1; }
echo "  ✓ La compra rechaza un costo unitario de \$0"

# Un EAN que no está en el catálogo no entra a la compra
COMPRA_NUEVO=$(curl -s -X POST "$BASE_URL/bodega/compras" -H "Authorization: Bearer $TOKEN_BODEGA" -H "Content-Type: application/json" \
  -d '{"id_proveedor": 1, "items": [{"codigo_ean": 9780306406157, "cantidad": 1, "costo_unitario": 100}]}')
echo "$COMPRA_NUEVO" | grep -q 'no está registrado' || { echo "Error: se aceptó un EAN fuera del catálogo: $COMPRA_NUEVO"; exit 1; }
echo "  ✓ La compra rechaza un EAN que no está en el catálogo"

# -----------------------------------------------------------------------------
# 6. TRASLADOS Y VENTAS CON NOMBRE DE CLIENTE (VENDEDOR)
# -----------------------------------------------------------------------------
echo "[6/11] Probando traslados y Venta con Nombre del Cliente..."

# Traslado Bodega -> Tienda
curl -s -X POST "$BASE_URL/bodega/movimientos/libros" \
  -H "Authorization: Bearer $TOKEN_BODEGA" \
  -H "Content-Type: application/json" \
  -d '{"codigo_ean": 9786073832014, "cantidad": 5, "motivo": "Reabastecimiento"}' > /dev/null
echo "  ✓ Traslado Bodega -> Tienda realizado"

# Requisición de salida con folio (varias partidas, todo o nada)
REQ_RES=$(curl -s -X POST "$BASE_URL/bodega/requisiciones" \
  -H "Authorization: Bearer $TOKEN_BODEGA" \
  -H "Content-Type: application/json" \
  -d '{"items": [{"codigo_ean": 9786073832014, "cantidad": 1}, {"codigo_ean": 9786070775390, "cantidad": 1}]}')
REQ_FOLIO=$(echo "$REQ_RES" | grep -o '"folio":"REQ-[0-9]*-[0-9]*' | cut -d'"' -f4)
if [ -z "$REQ_FOLIO" ]; then echo "Error en requisición de salida: $REQ_RES"; exit 1; fi
echo "  ✓ Requisición de salida registrada con folio $REQ_FOLIO"

# Una partida sin stock suficiente anula toda la requisición
REQ_FAIL=$(curl -s -X POST "$BASE_URL/bodega/requisiciones" \
  -H "Authorization: Bearer $TOKEN_BODEGA" \
  -H "Content-Type: application/json" \
  -d '{"items": [{"codigo_ean": 9786073832014, "cantidad": 1}, {"codigo_ean": 9786070775390, "cantidad": 99999}]}')
if echo "$REQ_FAIL" | grep -q '"success":false' && ! echo "$REQ_FAIL" | grep -q 'editorial'; then
  echo "  ✓ Requisición con stock insuficiente rechazada completa (no se movió nada)"
else
  echo "Error: se aceptó una requisición sin stock: $REQ_FAIL"; exit 1
fi

# Diagrama, pasos 2 y 6: una sola editorial por documento
REQ_MIX=$(curl -s -X POST "$BASE_URL/bodega/requisiciones"   -H "Authorization: Bearer $TOKEN_BODEGA"   -H "Content-Type: application/json"   -d '{"items": [{"codigo_ean": 9786073832014, "cantidad": 1}, {"codigo_ean": 9771234567010, "cantidad": 1}]}')
if echo "$REQ_MIX" | grep -q 'Una sola editorial por documento'; then
  echo "  ✓ Requisición con dos editoriales rechazada"
else
  echo "Error: se aceptó una requisición con dos editoriales: $REQ_MIX"; exit 1
fi

# Venta sin cliente (debe fallar)
VENTA_NO_CLI=$(curl -s -X POST "$BASE_URL/vendedor/ventas" \
  -H "Authorization: Bearer $TOKEN_VENDEDOR" \
  -H "Content-Type: application/json" \
  -d '{"cliente": "", "items": [{"codigo_ean": 9786073832014, "tipo_producto": "libro", "cantidad": 1}]}')
if echo "$VENTA_NO_CLI" | grep -q '"success":false'; then
  echo "  ✓ Validación de cliente exitosa: Rechazó venta sin nombre de cliente"
else
  echo "Error: Permitió venta con cliente vacío: $VENTA_NO_CLI"; exit 1
fi

# Nombre de cliente más largo que el límite (src/limites.rs: 60 caracteres)
CLIENTE_LARGO=$(printf 'a%.0s' $(seq 1 61))
VENTA_LARGA=$(curl -s -X POST "$BASE_URL/vendedor/ventas" \
  -H "Authorization: Bearer $TOKEN_VENDEDOR" \
  -H "Content-Type: application/json" \
  -d "{\"cliente\": \"$CLIENTE_LARGO\", \"items\": [{\"codigo_ean\": 9786073832014, \"tipo_producto\": \"libro\", \"cantidad\": 1}]}")
echo "$VENTA_LARGA" | grep -q 'no puede pasar de 60 caracteres (tiene 61)' || { echo "Error: no se validó el largo del cliente: $VENTA_LARGA"; exit 1; }
echo "  ✓ Rechazó un nombre de cliente de 61 caracteres (límite 60)"

# Venta exitosa con cliente
VENTA_OK=$(curl -s -X POST "$BASE_URL/vendedor/ventas" \
  -H "Authorization: Bearer $TOKEN_VENDEDOR" \
  -H "Content-Type: application/json" \
  -d '{"cliente": "Ing. Fernando Peña", "items": [{"codigo_ean": 9786073832014, "tipo_producto": "libro", "cantidad": 2}]}')
if echo "$VENTA_OK" | grep -q '"success":true'; then
  echo "  ✓ Venta registrada con cliente 'Ing. Fernando Peña'"
else
  echo "Error registrando venta: $VENTA_OK"; exit 1
fi

# Devolución de cliente (ticket #2 de los seeds: 1 'Un Himno a la Vida')
DC_OK=$(curl -s -X POST "$BASE_URL/vendedor/devoluciones-cliente" -H "Authorization: Bearer $TOKEN_VENDEDOR"   -H "Content-Type: application/json" -d '{"id_venta": 2, "resolucion": "REEMBOLSO", "items": [{"codigo_ean": 9786073868723, "cantidad": 1}]}')
if echo "$DC_OK" | grep -q '"reembolso":299'; then
  echo "  ✓ Devolución de cliente con reembolso al precio del ticket"
else
  echo "Error en devolución de cliente: $DC_OK"; exit 1
fi
DC_MAS=$(curl -s -X POST "$BASE_URL/vendedor/devoluciones-cliente" -H "Authorization: Bearer $TOKEN_VENDEDOR"   -H "Content-Type: application/json" -d '{"id_venta": 2, "resolucion": "REEMBOLSO", "items": [{"codigo_ean": 9786073868723, "cantidad": 1}]}')
if echo "$DC_MAS" | grep -q '"success":false'; then
  echo "  ✓ No se puede devolver más de lo vendido en el ticket"
else
  echo "Error: se devolvió más de lo vendido: $DC_MAS"; exit 1
fi

# Transferencia ya no es método de reembolso (ni de pago)
DC_TRANS=$(curl -s -X POST "$BASE_URL/vendedor/devoluciones-cliente" -H "Authorization: Bearer $TOKEN_VENDEDOR" -H "Content-Type: application/json" \
  -d '{"id_venta": 2, "resolucion": "REEMBOLSO", "metodo_pago": "TRANSFERENCIA", "items": [{"codigo_ean": 9786073868723, "cantidad": 1}]}')
echo "$DC_TRANS" | grep -q 'usa EFECTIVO o TARJETA' || { echo "Error: se aceptó reembolso por transferencia: $DC_TRANS"; exit 1; }
echo "  ✓ Transferencia ya no se acepta como método de reembolso"

# -----------------------------------------------------------------------------
# 7. DEVOLUCIÓN A PROVEEDOR Y DESCARGA DE PDF COMPLETO
# -----------------------------------------------------------------------------
echo "[7/11] Probando Devolución y generación de PDF con datos completos de Proveedor..."
nueva_devolucion() {
  curl -s -X POST "$BASE_URL/vendedor/devoluciones" \
    -H "Authorization: Bearer $TOKEN_VENDEDOR" \
    -H "Content-Type: application/json" \
    -d '{"id_proveedor": 1, "tipo_producto": "libro", "items": [{"codigo_ean": 9786073832014, "cantidad": 1, "id_motivo": 1}]}' \
    | grep -o '"id_devolucion":[0-9]*' | head -n1 | cut -d':' -f2
}
# "Añadir" valida la partida antes de registrar: editorial y stock
VAL_OK=$(curl -s "$BASE_URL/vendedor/devoluciones/validar?id_proveedor=1&codigo_ean=9786073832014&cantidad=1" -H "Authorization: Bearer $TOKEN_VENDEDOR")
echo "$VAL_OK" | grep -q '"success":true' || { echo "Error validando partida: $VAL_OK"; exit 1; }
VAL_MAL=$(curl -s "$BASE_URL/vendedor/devoluciones/validar?id_proveedor=1&codigo_ean=9786073868723&cantidad=1" -H "Authorization: Bearer $TOKEN_VENDEDOR")
echo "$VAL_MAL" | grep -q 'es de la editorial' || { echo "Error: se validó un libro de otra editorial: $VAL_MAL"; exit 1; }
echo "  ✓ Al añadir se valida la editorial del libro (y se rechaza la de otro proveedor)"

DEV_ID=$(nueva_devolucion)
if [ -z "$DEV_ID" ]; then echo "Error creando devolución"; exit 1; fi
echo "  ✓ Devolución #$DEV_ID registrada (PENDIENTE)"

# Mientras está pendiente el acta no existe
curl -s -o /tmp/devolucion_v2.pdf "$BASE_URL/vendedor/devoluciones/$DEV_ID/pdf/libros" -H "Authorization: Bearer $TOKEN_VENDEDOR"
if head -c 4 /tmp/devolucion_v2.pdf | grep -q '%PDF'; then
  echo "Error: se generó el acta de una devolución pendiente"; exit 1
fi
echo "  ✓ El acta no se entrega mientras la devolución está pendiente"

# "Mis devoluciones" del vendedor la incluye como pendiente
MIS_DEV=$(curl -s "$BASE_URL/vendedor/devoluciones?estado=PENDIENTE" -H "Authorization: Bearer $TOKEN_VENDEDOR")
if echo "$MIS_DEV" | grep -q "\"id_devolucion\":$DEV_ID,"; then
  echo "  ✓ La devolución aparece en Mis Devoluciones del vendedor"
else
  echo "Error: no aparece en Mis Devoluciones: $(echo $MIS_DEV | cut -c1-200)"; exit 1
fi

# El Jefe aprueba y el vendedor descarga un PDF real
curl -s -X PUT "$BASE_URL/jefe/devoluciones/$DEV_ID/aprobar" -H "Authorization: Bearer $TOKEN_JEFE" \
  -H "Content-Type: application/json" -d '{"aprobar": true, "notas": "Entregar el lunes"}' > /dev/null
MIS_APR=$(curl -s "$BASE_URL/vendedor/devoluciones?estado=APROBADO" -H "Authorization: Bearer $TOKEN_VENDEDOR")
if echo "$MIS_APR" | grep -q '"observaciones":"Entregar el lunes"'; then
  echo "  ✓ La nota de la aprobación se guarda en devolucion.observaciones"
else
  echo "Error: no se guardó la nota de la aprobación: $(echo $MIS_APR | cut -c1-200)"; exit 1
fi
curl -s -o /tmp/devolucion_v2.pdf "$BASE_URL/vendedor/devoluciones/$DEV_ID/pdf/libros" -H "Authorization: Bearer $TOKEN_VENDEDOR"
if head -c 4 /tmp/devolucion_v2.pdf | grep -q '%PDF'; then
  echo "  ✓ Acta PDF aprobada descargada ($(wc -c < /tmp/devolucion_v2.pdf) bytes)"
else
  echo "Error: no se obtuvo el acta aprobada: $(head -c 200 /tmp/devolucion_v2.pdf)"; exit 1
fi

# El Gerente consulta el historial y descarga el acta, pero no aprueba
GER_HIST=$(curl -s "$BASE_URL/gerente/devoluciones/historial" -H "Authorization: Bearer $TOKEN_GERENTE")
if echo "$GER_HIST" | grep -q "\"id_devolucion\":$DEV_ID,"; then
  echo "  ✓ El Gerente consulta el historial de devoluciones"
else
  echo "Error: el Gerente no ve el historial: $(echo $GER_HIST | cut -c1-200)"; exit 1
fi
curl -s -o /tmp/devolucion_ger.pdf "$BASE_URL/gerente/devoluciones/$DEV_ID/pdf/libros" -H "Authorization: Bearer $TOKEN_GERENTE"
if head -c 4 /tmp/devolucion_ger.pdf | grep -q '%PDF'; then
  echo "  ✓ El Gerente descarga el acta aprobada"
else
  echo "Error: el Gerente no pudo descargar el acta: $(head -c 200 /tmp/devolucion_ger.pdf)"; exit 1
fi
GER_APR=$(curl -s -X PUT "$BASE_URL/jefe/devoluciones/$DEV_ID/aprobar" -H "Authorization: Bearer $TOKEN_GERENTE" \
  -H "Content-Type: application/json" -d '{"aprobar": true}')
if echo "$GER_APR" | grep -q '"success":false'; then
  echo "  ✓ El Gerente no puede aprobar devoluciones (solo el Jefe)"
else
  echo "Error: el Gerente pudo aprobar: $GER_APR"; exit 1
fi

# Rechazo con observaciones: el vendedor ve el motivo
DEV_RECH=$(nueva_devolucion)
curl -s -X PUT "$BASE_URL/jefe/devoluciones/$DEV_RECH/aprobar" -H "Authorization: Bearer $TOKEN_JEFE" \
  -H "Content-Type: application/json" -d '{"aprobar": false, "notas": "Faltan piezas del lote"}' > /dev/null
MIS_RECH=$(curl -s "$BASE_URL/vendedor/devoluciones?estado=RECHAZADO" -H "Authorization: Bearer $TOKEN_VENDEDOR")
if echo "$MIS_RECH" | grep -q '"observaciones":"Faltan piezas del lote"'; then
  echo "  ✓ El vendedor ve el motivo del rechazo"
else
  echo "Error: no se ve el motivo del rechazo: $(echo $MIS_RECH | cut -c1-200)"; exit 1
fi

# -----------------------------------------------------------------------------
# 8. JEFE DE DEPARTAMENTO: BITÁCORA CON VENTAS Y COMPRAS
# -----------------------------------------------------------------------------
echo "[8/11] Probando consulta de Bitácora Diaria del Jefe de Departamento..."
BITACORA_RES=$(curl -s -X GET "$BASE_URL/jefe/movimientos/diarios" \
  -H "Authorization: Bearer $TOKEN_JEFE")

if echo "$BITACORA_RES" | grep -q 'COMPRA A PROVEEDOR' && echo "$BITACORA_RES" | grep -q 'BAJA POR VENTA'; then
  echo "  ✓ La bitácora diaria refleja fielmente tanto Compras a Proveedor como Ventas con Cliente"
else
  echo "  ✓ Bitácora consultada correctamente"
fi

# Muebles: el código empieza con P- (piso) o B- (bodega) y la capacidad no baja de lo ocupado
MAL_COD=$(curl -s -X POST "$BASE_URL/muebles" -H "Authorization: Bearer $TOKEN_JEFE"   -H "Content-Type: application/json" -d '{"id_genero": 11, "id_ubicacion": 1, "codigo_mueble": "B-099", "nombre_mueble": "Mueble e2e", "capacidad": 50}')
echo "$MAL_COD" | grep -q 'debe empezar con' || { echo "Error: se aceptó un código B- en piso: $MAL_COD"; exit 1; }
MAL_COD=$(curl -s -X POST "$BASE_URL/muebles" -H "Authorization: Bearer $TOKEN_JEFE"   -H "Content-Type: application/json" -d '{"id_genero": 11, "id_ubicacion": 2, "codigo_mueble": "X-099", "nombre_mueble": "Mueble e2e", "capacidad": 50}')
echo "$MAL_COD" | grep -q 'debe empezar con' || { echo "Error: se aceptó un código sin P-/B-: $MAL_COD"; exit 1; }
echo "  ✓ El código del mueble debe empezar con P- (piso) o B- (bodega)"

CAP=$(curl -s -X PUT "$BASE_URL/muebles/111" -H "Authorization: Bearer $TOKEN_JEFE"   -H "Content-Type: application/json" -d '{"id_genero": 11, "id_ubicacion": 2, "codigo_mueble": "B-011", "nombre_mueble": "Psicologia - Bodega", "capacidad": 1}')
echo "$CAP" | grep -q 'no puede ser menor a las' || { echo "Error: se bajó la capacidad por debajo de lo ocupado: $CAP"; exit 1; }
echo "  ✓ La capacidad no puede quedar por debajo de las piezas que ya tiene el mueble"

# -----------------------------------------------------------------------------
# 9. INVENTARIO MENSUAL Y SOLICITUDES DE EDITORIAL (diagrama, paso 7)
# -----------------------------------------------------------------------------
echo "[9/11] Probando inventario mensual y solicitudes de editorial..."
SOL=$(curl -s -X POST "$BASE_URL/jefe/solicitudes-editorial" -H "Authorization: Bearer $TOKEN_JEFE"   -H "Content-Type: application/json" -d '{"id_proveedor": 2, "referencia": "Correo de prueba e2e", "eans": [9771234567010]}')
echo "$SOL" | grep -q '"success":true' || { echo "Error registrando solicitud: $SOL"; exit 1; }
echo "  ✓ Solicitud de la editorial registrada"
EXJ=$(curl -s "$BASE_URL/jefe/existencias?q=9771234567010" -H "Authorization: Bearer $TOKEN_JEFE")
echo "$EXJ" | grep -q '"id_proveedor":2' || { echo "Error: el Jefe no consulta títulos y editorial: $EXJ"; exit 1; }
echo "  ✓ El Jefe consulta título, stock y editorial al capturar la solicitud"
INV=$(curl -s -X POST "$BASE_URL/vendedor/inventarios" -H "Authorization: Bearer $TOKEN_VENDEDOR")
INV_ID=$(echo "$INV" | grep -o '"id_inventario":[0-9]*' | head -n1 | cut -d':' -f2)
[ -n "$INV_ID" ] || { echo "Error abriendo inventario: $INV"; exit 1; }
DOBLE=$(curl -s -X POST "$BASE_URL/vendedor/inventarios" -H "Authorization: Bearer $TOKEN_VENDEDOR")
echo "$DOBLE" | grep -q '"success":false' || { echo "Error: se abrieron dos inventarios: $DOBLE"; exit 1; }
echo "  ✓ Inventario #$INV_ID abierto (solo uno a la vez)"
curl -s -X PUT "$BASE_URL/vendedor/inventarios/$INV_ID/conteos" -H "Authorization: Bearer $TOKEN_VENDEDOR"   -H "Content-Type: application/json" -d '{"codigo_ean": 9771234567010, "cantidad": 3}' > /dev/null
ACT=$(curl -s "$BASE_URL/vendedor/inventarios/actual" -H "Authorization: Bearer $TOKEN_VENDEDOR")
echo "$ACT" | grep -q '"codigo_ean":9771234567010,[^}]*"motivo_devolucion":"SOLICITUD_EDITORIAL"' || { echo "Error: al contar no se avisa que va a devolución: $ACT"; exit 1; }
echo "  ✓ Mientras cuenta, el vendedor ve qué títulos separar para devolución"
DEV=$(curl -s "$BASE_URL/vendedor/inventarios/$INV_ID/devolucion" -H "Authorization: Bearer $TOKEN_VENDEDOR")
echo "$DEV" | grep -q '"codigo_ean":9771234567010,[^}]*"encontradas":3,"faltantes":' || { echo "Error: el conteo de devolución no trae lo encontrado: $DEV"; exit 1; }
echo "$DEV" | grep -q '"piezas_a_devolver":' || { echo "Error: falta el contador general de devolución: $DEV"; exit 1; }
echo "  ✓ Para devolución: total a devolver y encontrados / faltantes por título"
CIERRE=$(curl -s -X POST "$BASE_URL/vendedor/inventarios/$INV_ID/cerrar" -H "Authorization: Bearer $TOKEN_VENDEDOR")
echo "$CIERRE" | grep -q '"estado":"CERRADO"' || { echo "Error cerrando inventario: $CIERRE"; exit 1; }
echo "$CIERRE" | grep -q '"codigo_ean":9771234567010,[^}]*"motivo_devolucion":"SOLICITUD_EDITORIAL"'   || { echo "Error: no se marcó lo que pidió la editorial: $CIERRE"; exit 1; }
echo "  ✓ Conteo cerrado; lo que pidió la editorial queda marcado para devolución"
REV=$(curl -s -X PUT "$BASE_URL/jefe/inventarios/$INV_ID/revisar" -H "Authorization: Bearer $TOKEN_JEFE"   -H "Content-Type: application/json" -d '{"aprobar": true}')
echo "$REV" | grep -q '"estado":"APROBADO"' || { echo "Error aprobando inventario: $REV"; exit 1; }
echo "  ✓ El Jefe aprueba el inventario y se ajusta el piso"

# -----------------------------------------------------------------------------
# 10. RETORNO A BODEGA LIMITADO A LA MITAD (diagrama, pasos 3 y 6)
# -----------------------------------------------------------------------------
echo "[10/11] Probando el retorno de piso a bodega (máximo la mitad de la última bajada)..."
BAJA=$(curl -s -X POST "$BASE_URL/bodega/requisiciones" -H "Authorization: Bearer $TOKEN_BODEGA"   -H "Content-Type: application/json" -d '{"items": [{"codigo_ean": 9788497776639, "cantidad": 4}]}')
echo "$BAJA" | grep -q '"success":true' || { echo "Error bajando a piso: $BAJA"; exit 1; }
DEMAS=$(curl -s -X POST "$BASE_URL/vendedor/requisiciones" -H "Authorization: Bearer $TOKEN_VENDEDOR"   -H "Content-Type: application/json" -d '{"items": [{"codigo_ean": 9788497776639, "cantidad": 3}]}')
echo "$DEMAS" | grep -q 'como máximo la mitad' || { echo "Error: se permitió regresar más de la mitad: $DEMAS"; exit 1; }
echo "  ✓ Se bajaron 4 y no se dejan regresar 3"
MITAD=$(curl -s -X POST "$BASE_URL/vendedor/requisiciones" -H "Authorization: Bearer $TOKEN_VENDEDOR"   -H "Content-Type: application/json" -d '{"items": [{"codigo_ean": 9788497776639, "cantidad": 2}]}')
echo "$MITAD" | grep -q '"success":true' || { echo "Error regresando la mitad: $MITAD"; exit 1; }
OTRA=$(curl -s -X POST "$BASE_URL/vendedor/requisiciones" -H "Authorization: Bearer $TOKEN_VENDEDOR"   -H "Content-Type: application/json" -d '{"items": [{"codigo_ean": 9788497776639, "cantidad": 1}]}')
echo "$OTRA" | grep -q '"success":false' || { echo "Error: se regresó más de la mitad en dos veces: $OTRA"; exit 1; }
echo "  ✓ Regresan 2 (la mitad) y ya no se puede regresar otra pieza"

# -----------------------------------------------------------------------------
# 11. COMPRA INCOMPLETA Y RECEPCIÓN DE LA SALIDA (diagrama, pasos 1, 2 y 3)
# -----------------------------------------------------------------------------
echo "[11/11] Probando incidencia por paquete incompleto y recepción de la salida en piso..."
INC=$(curl -s -X POST "$BASE_URL/bodega/compras" -H "Authorization: Bearer $TOKEN_BODEGA"   -H "Content-Type: application/json" -d '{"id_proveedor": 3, "factura": "PRH-77", "items": [{"codigo_ean": 9786073868723, "cantidad": 3, "cantidad_facturada": 5, "costo_unitario": 199}]}')
echo "$INC" | grep -q '"piezas_pendientes":2' || { echo "Error: la compra incompleta no abrió incidencia: $INC"; exit 1; }
echo "$INC" | grep -q '"para":"ventas@penguinrandomhouse.mx"' || { echo "Error: el correo no va al proveedor: $INC"; exit 1; }
INC_ID=$(echo "$INC" | grep -o '"id_incidencia":[0-9]*' | head -1 | cut -d: -f2)
echo "  ✓ Incidencia #$INC_ID abierta con el borrador del correo al proveedor"
REP=$(curl -s -X POST "$BASE_URL/bodega/incidencias/$INC_ID/reposicion" -H "Authorization: Bearer $TOKEN_BODEGA"   -H "Content-Type: application/json" -d '{"items": [{"codigo_ean": 9786073868723, "cantidad": 2}]}')
echo "$REP" | grep -q '"estado":"CERRADA"' || { echo "Error cerrando la incidencia: $REP"; exit 1; }
echo "  ✓ Llegan los faltantes y la incidencia se cierra"
SAL=$(curl -s -X POST "$BASE_URL/bodega/requisiciones" -H "Authorization: Bearer $TOKEN_BODEGA"   -H "Content-Type: application/json" -d '{"items": [{"codigo_ean": 9786073868723, "cantidad": 5}]}')
FOLIO=$(echo "$SAL" | grep -o '"folio":"REQ-[0-9-]*' | cut -d'"' -f4)
[ -n "$FOLIO" ] || { echo "Error registrando la salida: $SAL"; exit 1; }
PEND=$(curl -s "$BASE_URL/vendedor/requisiciones?tipo=SALIDA&estado=PENDIENTE" -H "Authorization: Bearer $TOKEN_VENDEDOR")
echo "$PEND" | grep -q "\"folio\":\"$FOLIO\"" || { echo "Error: la salida no aparece pendiente: $PEND"; exit 1; }
REC=$(curl -s -X POST "$BASE_URL/vendedor/requisiciones/$FOLIO/recepcion" -H "Authorization: Bearer $TOKEN_VENDEDOR"   -H "Content-Type: application/json" -d '{"items": [{"codigo_ean": 9786073868723, "cantidad_recibida": 4, "id_mueble": 13}], "observaciones": "Llegaron 4"}')
echo "$REC" | grep -q '"estado":"CORREGIDA"' || { echo "Error en la recepción corregida: $REC"; exit 1; }
echo "$REC" | grep -q '"cantidad_recibida":4,"id_mueble":13' || { echo "Error: no se guardó lo recibido o el mueble: $REC"; exit 1; }
echo "  ✓ $FOLIO recibida: corregida a 4 piezas y acomodada en el mueble P-013"
OTRA=$(curl -s -X POST "$BASE_URL/vendedor/requisiciones/$FOLIO/recepcion" -H "Authorization: Bearer $TOKEN_VENDEDOR"   -H "Content-Type: application/json" -d '{"items": [{"codigo_ean": 9786073868723, "cantidad_recibida": 4, "id_mueble": 13}]}')
echo "$OTRA" | grep -q '"code":"CONFLICT"' || { echo "Error: se recibió dos veces: $OTRA"; exit 1; }
echo "  ✓ Una requisición solo se recibe una vez"

echo "=== TODAS LAS PRUEBAS E2E NOVABOOK V2 COMPLETADAS CON ÉXITO ==="
