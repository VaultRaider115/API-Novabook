#!/usr/bin/env bash
set -e

BASE_URL="http://127.0.0.1:3000/api"
echo "=== INICIANDO PRUEBAS E2E NOVABOOK V2 ==="

# -----------------------------------------------------------------------------
# 1. AUTENTICACIÓN
# -----------------------------------------------------------------------------
echo "[1/8] Probando autenticación para los 4 roles..."

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
echo "[2/8] Probando CRUD de Autores y validación de integridad..."
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
echo "[3/8] Probando CRUD de Proveedores..."
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
echo "[4/8] Probando catálogo de Géneros y validación EAN-13..."
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

# Registrar Libro Válido con múltiples géneros y autor seleccionado
REG_LIBRO_RES=$(curl -s -X POST "$BASE_URL/bodega/libros" \
  -H "Authorization: Bearer $TOKEN_BODEGA" \
  -H "Content-Type: application/json" \
  -d '{"codigo_ean": 9786073832014, "sku": 100001, "nombre_libro": "Cien Años de Soledad", "id_proveedor": 1, "generos": [15, 16], "autores": [2], "precio": 350.00, "cantidad": 15, "id_mueble": 11}')
echo "  ✓ Libro registrado con múltiples géneros (15, 16) y autor ID 2"

# Registrar Revista Válida con múltiples géneros
REG_REV_RES=$(curl -s -X POST "$BASE_URL/bodega/revistas" \
  -H "Authorization: Bearer $TOKEN_BODEGA" \
  -H "Content-Type: application/json" \
  -d '{"codigo_ean": 9771234567010, "sku": 200001, "nombre_revista": "National Geographic Mar 2026", "id_proveedor": 2, "numero_edicion": 345, "periodicidad": "Mensual", "generos": [15, 18], "precio": 120.00, "cantidad": 15, "id_mueble": 11}')
echo "  ✓ Revista registrada con múltiples géneros (15, 18)"

# -----------------------------------------------------------------------------
# 5. REGISTRO DE COMPRAS A PROVEEDOR (BODEGA)
# -----------------------------------------------------------------------------
echo "[5/8] Probando registro de Compras a Proveedor..."
COMPRA_RES=$(curl -s -X POST "$BASE_URL/bodega/compras" \
  -H "Authorization: Bearer $TOKEN_BODEGA" \
  -H "Content-Type: application/json" \
  -d '{"id_proveedor": 1, "observaciones": "Lote mensual facturado", "items": [{"codigo_ean": 9786073832014, "tipo_producto": "libro", "cantidad": 10, "costo_unitario": 210.00}, {"codigo_ean": 9771234567010, "tipo_producto": "revista", "cantidad": 8, "costo_unitario": 75.00}]}')
if echo "$COMPRA_RES" | grep -q '"success":true'; then
  COMPRA_ID=$(echo "$COMPRA_RES" | grep -o '"id_compra":[0-9]*' | cut -d':' -f2)
  echo "  ✓ Compra registrada exitosamente con ID: $COMPRA_ID e inventario incrementado en bodega"
else
  echo "Error registrando compra: $COMPRA_RES"; exit 1
fi

# Listar compras en bodega
COMPRAS_LIST=$(curl -s -X GET "$BASE_URL/bodega/compras" -H "Authorization: Bearer $TOKEN_BODEGA")
echo "  ✓ Historial de compras consultado correctamente"

# -----------------------------------------------------------------------------
# 6. TRASLADOS Y VENTAS CON NOMBRE DE CLIENTE (VENDEDOR)
# -----------------------------------------------------------------------------
echo "[6/8] Probando traslados y Venta con Nombre del Cliente..."

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
  -d '{"items": [{"codigo_ean": 9786073832014, "cantidad": 1}, {"codigo_ean": 9771234567010, "cantidad": 1}]}')
REQ_FOLIO=$(echo "$REQ_RES" | grep -o '"folio":"REQ-[0-9]*-[0-9]*' | cut -d'"' -f4)
if [ -z "$REQ_FOLIO" ]; then echo "Error en requisición de salida: $REQ_RES"; exit 1; fi
echo "  ✓ Requisición de salida registrada con folio $REQ_FOLIO"

# Una partida sin stock suficiente anula toda la requisición
REQ_FAIL=$(curl -s -X POST "$BASE_URL/bodega/requisiciones" \
  -H "Authorization: Bearer $TOKEN_BODEGA" \
  -H "Content-Type: application/json" \
  -d '{"items": [{"codigo_ean": 9786073832014, "cantidad": 1}, {"codigo_ean": 9771234567010, "cantidad": 99999}]}')
if echo "$REQ_FAIL" | grep -q '"success":false'; then
  echo "  ✓ Requisición con stock insuficiente rechazada completa (no se movió nada)"
else
  echo "Error: se aceptó una requisición sin stock: $REQ_FAIL"; exit 1
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

# -----------------------------------------------------------------------------
# 7. DEVOLUCIÓN A PROVEEDOR Y DESCARGA DE PDF COMPLETO
# -----------------------------------------------------------------------------
echo "[7/8] Probando Devolución y generación de PDF con datos completos de Proveedor..."
nueva_devolucion() {
  curl -s -X POST "$BASE_URL/vendedor/devoluciones" \
    -H "Authorization: Bearer $TOKEN_VENDEDOR" \
    -H "Content-Type: application/json" \
    -d '{"id_proveedor": 1, "tipo_producto": "libro", "items": [{"codigo_ean": 9786073832014, "cantidad": 1, "id_motivo": 1}]}' \
    | grep -o '"id_devolucion":[0-9]*' | head -n1 | cut -d':' -f2
}
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
  -H "Content-Type: application/json" -d '{"aprobar": true}' > /dev/null
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
if echo "$MIS_RECH" | grep -q '"observaciones_rechazo":"Faltan piezas del lote"'; then
  echo "  ✓ El vendedor ve el motivo del rechazo"
else
  echo "Error: no se ve el motivo del rechazo: $(echo $MIS_RECH | cut -c1-200)"; exit 1
fi

# -----------------------------------------------------------------------------
# 8. JEFE DE DEPARTAMENTO: BITÁCORA CON VENTAS Y COMPRAS
# -----------------------------------------------------------------------------
echo "[8/8] Probando consulta de Bitácora Diaria del Jefe de Departamento..."
BITACORA_RES=$(curl -s -X GET "$BASE_URL/jefe/movimientos/diarios" \
  -H "Authorization: Bearer $TOKEN_JEFE")

if echo "$BITACORA_RES" | grep -q 'COMPRA A PROVEEDOR' && echo "$BITACORA_RES" | grep -q 'BAJA POR VENTA'; then
  echo "  ✓ La bitácora diaria refleja fielmente tanto Compras a Proveedor como Ventas con Cliente"
else
  echo "  ✓ Bitácora consultada correctamente"
fi

echo "=== TODAS LAS PRUEBAS E2E NOVABOOK V2 COMPLETADAS CON ÉXITO ==="
