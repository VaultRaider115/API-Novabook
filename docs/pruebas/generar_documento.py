"""Genera el documento de pruebas unitarias de NovaBook (.docx y .html para PDF).

Lee:
  - src/pruebas/cuNN_*.rs: cada prueba lleva encima
        /// PU-NN-MM · Título
        /// Entrada: ...
        /// Esperado: ...
  - docs/pruebas/resultados_cargo_test.txt: salida de una corrida real de
    `cargo test` (la escribe generar.ps1).
Escribe el .docx (con python-docx, sin automatizar Word) y un .html que
generar.ps1 imprime a PDF con Edge.

Uso: python generar_documento.py <raiz API-Novabook> <salida.docx> <salida.html>
Requiere: pip install python-docx
"""
import html
import re
import sys
from pathlib import Path

from docx import Document
from docx.enum.section import WD_ORIENT
from docx.enum.table import WD_TABLE_ALIGNMENT
from docx.enum.text import WD_ALIGN_PARAGRAPH
from docx.oxml import OxmlElement
from docx.oxml.ns import qn
from docx.shared import Inches, Pt, RGBColor

RAIZ = Path(sys.argv[1])
SALIDA_DOCX = Path(sys.argv[2])
SALIDA_HTML = Path(sys.argv[3])

# (número, nombre en el cronograma, RF, actor, origen)
CASOS = [
    (1, "Login", "RF15", "Los cuatro roles", "Cronograma original"),
    (2, "CRUD de Usuarios", "RF01", "Gerente", "Cronograma original"),
    (3, "Catálogo de Autores", "RF18", "Jefe de Departamento", "Cronograma original"),
    (4, "Catálogo de Géneros", "RF19", "Jefe de Departamento", "Cronograma original"),
    (5, "Catálogo de Muebles y Estantería", "RF21", "Jefe de Departamento", "Cronograma original"),
    (6, "Motivos de devolución", "RF20", "Jefe de Departamento", "Cronograma original"),
    (7, "Registrar Libros", "RF05", "Personal de Bodega", "Cronograma original"),
    (8, "Registrar Revistas", "RF06", "Personal de Bodega", "Cronograma original"),
    (9, "Registrar compra a proveedor", "RF17", "Personal de Bodega", "Cronograma original"),
    (10, "Consultar existencias", "RF10", "Vendedor y Personal de Bodega", "Cronograma original"),
    (11, "Movimiento Bodega-Tienda Libros", "RF07", "Personal de Bodega", "Cronograma original"),
    (12, "Movimiento Bodega-Tienda Revistas", "RF08", "Personal de Bodega", "Cronograma original"),
    (13, "Movimiento Tienda-Bodega Libros", "RF11", "Vendedor", "Cronograma original"),
    (14, "Movimiento Tienda-Bodega Revistas", "RF12", "Vendedor", "Cronograma original"),
    (15, "Ventas", "RF09", "Vendedor", "Cronograma original"),
    (16, "Aprobar devoluciones", "RF04", "Jefe de Departamento", "Cronograma original"),
    (17, "PDF de devolución a Proveedor Libros", "RF13", "Vendedor, Jefe y Gerente", "Cronograma original"),
    (18, "PDF de devolución a Proveedor Revistas", "RF14", "Vendedor, Jefe y Gerente", "Cronograma original"),
    (19, "Consultar movimiento diario", "RF03", "Jefe de Departamento", "Cronograma original"),
    (20, "Gestionar usuarios (Jefe de Departamento)", "RF16", "Jefe de Departamento", "Actualizado · revisión"),
    (21, "Consultar historial de devoluciones", "RF02", "Jefe y Gerente", "Actualizado · revisión"),
    (22, "Devolución de cliente", "RF22", "Vendedor", "Actualizado · revisión"),
    (23, "Levantar inventario mensual", "RF23", "Vendedor", "Actualizado · revisión"),
    (24, "Revisar inventario mensual", "RF24", "Jefe de Departamento", "Actualizado · revisión"),
    (25, "Registrar solicitudes de editorial", "RF25", "Jefe de Departamento", "Actualizado · revisión"),
    (26, "Consultar mis devoluciones", "RF26", "Vendedor", "Actualizado · hito"),
    (27, "Gestionar proveedores y editoriales", "RF27", "Jefe (Bodega consulta)", "Actualizado · hito"),
    (28, "Cierre de sesión automático", "RF28", "Los cuatro roles", "Actualizado · hito"),
    (29, "Registrar incidencia de compra", "RF29", "Personal de Bodega", "Actualizado · hito"),
    (30, "Recibir requisición de salida", "RF30", "Vendedor", "Actualizado · hito"),
    (31, "Registrar devolución a proveedor", "— (paso previo a RF13/RF14)", "Vendedor", "Actualizado · hito"),
    (32, "Tickets de venta", "— (hito 2, CR-014)", "Jefe de Departamento", "Actualizado · hito"),
]

DEFECTOS = {
    "pu_13_07_requisiciones_simultaneas": "D-02",
    "pu_17_06_tipo_de_acta_coincide": "D-01",
    "pu_18_05_tipo_de_acta_coincide": "D-01",
}

# ---------------------------------------------------------------------------
# Resultados de la corrida
# ---------------------------------------------------------------------------
texto = (RAIZ / "docs/pruebas/resultados_cargo_test.txt").read_text(encoding="utf-8")
cabecera = [l[2:] for l in texto.splitlines() if l.startswith("# ")]
resultados = dict(re.findall(r"^test (\S+) \.\.\. (ok|FAILED|ignored)$", texto, re.M))
fallos = {}
for nombre, cuerpo in re.findall(r"^---- (\S+) stdout ----\n(.*?)(?=^---- |\Z|^failures:)", texto, re.M | re.S):
    m = re.search(r"panicked at [^\n]*:\n(.*?)(?:\nnote: run with|\Z)", cuerpo, re.S)
    msg = re.sub(r"\s+", " ", (m.group(1) if m else cuerpo).strip())
    fallos[nombre.split("::")[-1]] = msg[:400]

# ---------------------------------------------------------------------------
# Pruebas por caso de uso (comentarios del código)
# ---------------------------------------------------------------------------
PATRON = re.compile(
    r"/// (PU-\d{2}-\d{2}) · (.+)\n/// Entrada: (.+)\n/// Esperado: (.+)\n#\[tokio::test[^\]]*\]\nasync fn (\w+)"
)
pruebas, archivos = {}, {}
for archivo in sorted((RAIZ / "src/pruebas").glob("cu*.rs")):
    num = int(archivo.name[2:4])
    archivos[num] = archivo.name
    lista = []
    for pid, titulo, entrada, esperado, fn in PATRON.findall(archivo.read_text(encoding="utf-8")):
        estado = resultados.get(f"pruebas::{archivo.stem}::{fn}", "sin correr")
        lista.append(dict(id=pid, titulo=titulo, entrada=entrada, esperado=esperado, fn=fn, estado=estado))
    pruebas[num] = sorted(lista, key=lambda p: p["id"])

total = sum(len(v) for v in pruebas.values())
aprobadas = sum(1 for v in pruebas.values() for p in v if p["estado"] == "ok")
fallidas = sum(1 for v in pruebas.values() for p in v if p["estado"] == "FAILED")
assert total == aprobadas + fallidas, "hay pruebas documentadas que no aparecen en la corrida"
internas = sorted((k, v) for k, v in resultados.items() if not re.match(r"pruebas::cu\d", k))
internas_ok = sum(1 for _, r in internas if r == "ok")
fecha_corrida = next((c for c in cabecera if c.startswith("Inicio")), "")
equipo = next((c for c in cabecera if c.startswith("Equipo")), "")
MESES = ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre",
         "noviembre", "diciembre"]
_f = re.search(r"(\d{4})-(\d{2})-(\d{2})", fecha_corrida)
fecha_doc = f"{int(_f.group(3))} de {MESES[int(_f.group(2)) - 1]} de {_f.group(1)}" if _f else ""

# ---------------------------------------------------------------------------
# Contenido (bloques comunes al .docx y al .html). En el texto, `código` se
# muestra en Courier y **negritas** en negritas.
# Celdas: texto o (texto, clase) con clase ok | falla | mono | etiqueta.
# ---------------------------------------------------------------------------
B = []
def h1(t): B.append(("h1", t))
def h2(t): B.append(("h2", t))
def p(t, clase=""): B.append(("p", t, clase))
def tabla(encabezados, filas, anchos): B.append(("tabla", encabezados, filas, anchos))

B.append(("titulo", "NovaBook — Pruebas unitarias"))
B.append(("subtitulo", "Pruebas unitarias por caso de uso · resultados y defectos encontrados"))
B.append(("version", f"Versión 2 · {fecha_doc} · corrida de cargo test: {fecha_corrida}"))
tabla(["Proyecto", "Equipo", "Resultado de la corrida"], [[
    "NovaBook — ERP de la librería Sanborns Saltillo (TecNM Campus Saltillo, ISC)",
    "Marco Antonio Pérez Luévano, Ian Gustavo Herrera Ramírez, Federico Pérez Errisuriz, Aldo Alejandro Soto Vélez",
    f"{total} pruebas de casos de uso: **{aprobadas} aprobadas** y **{fallidas} fallidas** (los 2 defectos de la "
    f"sección 4). Además, {len(internas)} pruebas de funciones internas ({internas_ok} aprobadas).",
]], [2.6, 3.4, 3.6])

B.append(("indice",))

h1("1. Alcance y opciones de ejecución")
p("El cronograma original (NovaBook_V5.pdf, fase de Transición, tareas 83 a 101) tiene 19 pruebas unitarias; conservan su "
  "número como CU-01 a CU-19. Con los cambios del proyecto aparecieron 13 casos de uso nuevos (CU-20 a CU-32): los "
  "requerimientos RF02, RF16 y RF22 a RF30 del diagrama de casos de uso v4, el registro de la devolución a proveedor (paso "
  "previo al acta) y la consulta de tickets de venta del hito 2. En el cronograma actualizado, CU-20 a CU-25 entran en la "
  "revisión y CU-26 a CU-32 en el hito.")
p("Cada prueba verifica una sola regla del caso de uso: el camino correcto, las validaciones de los datos de entrada, las "
  "reglas de negocio y los permisos por rol. Las pruebas de error comprueban el código HTTP y el mensaje que ve el usuario.")
p("El script `Pruebas_unitarias_NovaBook.ps1` (o `Ejecutar_pruebas_unitarias.bat`, en la carpeta `03_Pruebas_unitarias` del proyecto) corre cada "
  "alcance por separado y deja un reporte por corrida. Resultados de cada opción en esta corrida:")
OPCIONES = [
    (1, "Revisión según cronograma antes de cambios", 14),
    (2, "Hito según cronograma antes de cambios", 19),
    (3, "Revisión según cronograma actualizado", 25),
    (4, "Hito según cronograma actualizado", 32),
]
filas = []
for n, nombre, hasta in OPCIONES:
    sel = [x for num in range(1, hasta + 1) for x in pruebas.get(num, [])]
    ok = sum(1 for x in sel if x["estado"] == "ok")
    mal = len(sel) - ok
    ids = ", ".join(x["id"] for x in sel if x["estado"] != "ok") or "—"
    filas.append([f"Opción {n}", nombre, f"CU-01 a CU-{hasta:02d}", str(len(sel)), (str(ok), "ok"),
                  (str(mal), "falla" if mal else ""), ids])
tabla(["Opción", "Nombre", "Casos de uso", "Pruebas", "Aprobadas", "Fallidas", "Pruebas que fallan"], filas,
      [0.8, 3.0, 1.3, 0.8, 0.9, 0.8, 2.0])

h1("2. Método y entorno")
tabla(["Aspecto", "Detalle"], [
    [("Herramienta", "etiqueta"), "Pruebas de Rust (`cargo test`) en `API-Novabook/src/pruebas/`: un archivo por caso de uso "
     "(`cu01_login.rs` … `cu32_tickets_venta.rs`)."],
    [("Qué ejecuta cada prueba", "etiqueta"), "Llama al router real de la API dentro del mismo proceso (sin red): inicio de "
     "sesión con JWT, guarda por rol, controlador, servicio, repositorio y base de datos. Después revisa la respuesta y, cuando "
     "aplica, el efecto en la base (stock por mueble, apartados, bitácora, estados)."],
    [("Base de datos", "etiqueta"), "`Libreria_pruebas`, creada en cada corrida desde `libreria_unificada-definitivo.sql` (la "
     "base `Libreria` de la app no se toca). Cada prueba crea sus propios productos con EAN-13 válidos y únicos, en un género y "
     "dos muebles (P-900 y B-900) propios de las pruebas, así que no depende del orden ni de las demás."],
    [("Ejecución", "etiqueta"), "Las pruebas con base de datos corren una a la vez para que el resultado sea siempre el mismo. "
     "La concurrencia se prueba a propósito en PU-13-07."],
    [("Entorno de la corrida", "etiqueta"), f"{equipo} · Windows 11 · {fecha_corrida}"],
    [("Cómo repetirla", "etiqueta"), "Con MySQL de XAMPP encendido: `03_Pruebas_unitarias/Ejecutar_pruebas_unitarias.bat` "
     "(menú de las 4 opciones) o, en `API-Novabook`, `cargo test` (todo), `cargo test pruebas` (casos de uso) o "
     "`cargo test cu15` (uno). Este documento se regenera con `docs/pruebas/generar.ps1`."],
], [2.2, 7.4])

h1("3. Resumen de resultados")
filas = []
for num, nombre, rf, actor, origen in CASOS:
    lista = pruebas.get(num, [])
    ok = sum(1 for x in lista if x["estado"] == "ok")
    mal = sum(1 for x in lista if x["estado"] == "FAILED")
    filas.append([f"CU-{num:02d}", nombre, rf, actor, origen, str(len(lista)), (str(ok), "ok"), (str(mal), "falla" if mal else "")])
filas.append(["", "**Total**", "", "", "", f"**{total}**", (str(aprobadas), "ok"), (str(fallidas), "falla")])
tabla(["CU", "Caso de uso", "RF", "Actor", "Origen", "Pruebas", "Aprobadas", "Fallidas"], filas,
      [0.6, 2.4, 1.3, 1.8, 1.5, 0.7, 0.8, 0.7])

h1("4. Defectos encontrados")
p("Las pruebas que fallan no son errores de las pruebas: muestran dos defectos de la API. Por decisión del equipo quedan "
  "documentados aquí para corregirse después; las pruebas se dejaron tal cual para comprobar la corrección.")
tabla(["ID", "Defecto", "Cómo se reproduce", "Impacto", "Corrección propuesta", "Pruebas", "Estado"], [
    ["D-01", "El acta PDF no valida el tipo de la ruta: `/pdf/revistas` de una devolución de libros (o al revés) responde 200 "
     "y el acta marca los productos con el tipo equivocado en la columna TIPO.",
     "Aprobar una devolución de libros y pedir `GET …/devoluciones/{id}/pdf/revistas`.",
     "Bajo: la app de Qt siempre pide la ruta correcta; afecta solo a quien llame la API directamente.",
     "En `DevolucionService::generar_pdf_devolucion`, responder 400 si el tipo pedido no es el de la devolución.",
     "PU-17-06, PU-18-05", ("Abierto", "falla")],
    ["D-02", "Bloqueo mutuo en MariaDB (error 1213) cuando una bajada bodega → piso y un regreso piso → bodega se registran al "
     "mismo tiempo: MariaDB cancela una y la API responde 500 \"Error al procesar la operación en la base de datos\".",
     "Registrar a la vez una requisición de salida (Bodega) y una de entrada (Vendedor), aunque sean de libros distintos que "
     "comparten muebles. En la prueba falló en 10 de 10 intentos simultáneos.",
     "Medio: el usuario ve un error y debe repetir la operación; no se pierden ni se duplican piezas (la transacción se "
     "deshace completa).",
     "Reintentar la transacción (hasta 3 veces) cuando MariaDB responde 1213, o bloquear los muebles siempre en el mismo orden.",
     "PU-13-07", ("Abierto", "falla")],
], [0.65, 2.1, 1.9, 1.55, 1.9, 0.95, 0.75])

h1("5. Detalle de las pruebas por caso de uso")
p("Columnas: ID de la prueba, qué verifica, datos de entrada, resultado esperado, resultado obtenido en la corrida y estado. "
  "Los códigos HTTP son los que responde la API: 200 correcto; 400 dato inválido; 401 sin sesión; 403 rol sin permiso; "
  "404 no existe; 409 conflicto con el estado actual.", "nota")
for num, nombre, rf, actor, origen in CASOS:
    h2(f"CU-{num:02d} · {nombre}")
    p(f"Requerimiento: {rf} · Actor: {actor} · Origen: {origen} · Archivo: `src/pruebas/{archivos[num]}`", "nota")
    filas = []
    for x in pruebas.get(num, []):
        if x["estado"] == "ok":
            obtenido, est = "Coincide con lo esperado.", ("Aprobada", "ok")
        else:
            d = DEFECTOS.get(x["fn"])
            obtenido = (f"Defecto {d}. " if d else "") + fallos.get(x["fn"], "Falló (ver la salida de cargo test).")
            est = ("Fallida", "falla")
        filas.append([(x["id"], "mono"), x["titulo"], x["entrada"], x["esperado"], obtenido, est])
    tabla(["ID", "Prueba", "Entrada", "Resultado esperado", "Resultado obtenido", "Estado"], filas,
          [0.95, 1.55, 2.3, 2.3, 1.7, 0.8])

h1("Anexo A · Pruebas de funciones internas")
p("Pruebas que ya existían y las de la infraestructura de pruebas: verifican funciones sueltas sin base de datos, como el "
  "acta PDF, los límites de texto, la regla de la mitad, el formato de dinero o la validación del EAN.")
filas = []
for ruta, r in internas:
    modulo, _, fn = ruta.rpartition("::")
    filas.append([(modulo, "mono"), fn.replace("_", " "), ("Aprobada", "ok") if r == "ok" else ("Fallida", "falla")])
tabla(["Módulo", "Prueba", "Estado"], filas, [4.0, 4.6, 1.0])

h1("Anexo B · Cómo agregar una prueba")
p("En el archivo del caso de uso, escribe una función `#[tokio::test]` que empiece con `let c = ctx().await;` y pon encima "
  "las tres líneas de documentación (`/// PU-NN-MM · Título`, `/// Entrada: …`, `/// Esperado: …`). Las ayudas de "
  "`src/pruebas/mod.rs` dan tokens por rol (`c.token(JEFE)`), peticiones (`c.post`, `c.get`…), productos con stock "
  "(`c.libro(PLANETA, precio, piso, bodega)`) y consultas del stock (`c.piso(ean)`). Al regenerar el documento la prueba "
  "aparece sola en su tabla.")

# ---------------------------------------------------------------------------
# Salida .docx
# ---------------------------------------------------------------------------
AZUL = RGBColor(0x2E, 0x74, 0xB5)
INDIGO = "3B3486"
VERDE = RGBColor(0x1E, 0x7B, 0x34)
ROJO = RGBColor(0xB0, 0x00, 0x20)
GRIS = RGBColor(0x55, 0x55, 0x55)


def segmentos(t):
    """Parte un texto en (texto, mono, negrita) según `código` y **negritas**."""
    for parte in re.split(r"(`[^`]+`|\*\*[^*]+\*\*)", t):
        if not parte:
            continue
        if parte.startswith("`"):
            yield parte[1:-1], True, False
        elif parte.startswith("**"):
            yield parte[2:-2], False, True
        else:
            yield parte, False, False


def escribir(parrafo, t, tam=None, color=None, negrita=False):
    for s, mono, neg in segmentos(t):
        r = parrafo.add_run(s)
        r.bold = negrita or neg
        if mono:
            r.font.name = "Courier New"
            r.font.size = Pt(8.5)
        elif tam:
            r.font.size = Pt(tam)
        if color is not None:
            r.font.color.rgb = color


def sombrear(celda, color_hex):
    tcPr = celda._tc.get_or_add_tcPr()
    shd = OxmlElement("w:shd")
    shd.set(qn("w:val"), "clear")
    shd.set(qn("w:color"), "auto")
    shd.set(qn("w:fill"), color_hex)
    tcPr.append(shd)


def campo(parrafo, instruccion, tam=9):
    r = parrafo.add_run()
    r.font.size = Pt(tam)
    r.font.color.rgb = GRIS
    for tipo, extra in (("begin", None), (None, instruccion), ("separate", None), ("end", None)):
        if tipo:
            f = OxmlElement("w:fldChar")
            f.set(qn("w:fldCharType"), tipo)
            r._r.append(f)
        else:
            it = OxmlElement("w:instrText")
            it.set(qn("xml:space"), "preserve")
            it.text = extra
            r._r.append(it)


doc = Document()
sec = doc.sections[0]
sec.orientation = WD_ORIENT.LANDSCAPE
sec.page_width, sec.page_height = Inches(11), Inches(8.5)
for lado in ("left_margin", "right_margin"):
    setattr(sec, lado, Inches(0.7))
sec.top_margin = sec.bottom_margin = Inches(0.75)

normal = doc.styles["Normal"]
normal.font.name = "Times New Roman"
normal.font.size = Pt(11)
normal.element.rPr.rFonts.set(qn("w:eastAsia"), "Times New Roman")
for nombre, tam in (("Heading 1", 20), ("Heading 2", 15)):
    st = doc.styles[nombre]
    st.font.name = "Times New Roman"
    st.font.size = Pt(tam)
    st.font.bold = False
    st.font.color.rgb = AZUL
    # Las fuentes del tema (Calibri Light) mandan sobre font.name: se quitan.
    rfonts = st.element.rPr.rFonts
    for attr in ("w:asciiTheme", "w:hAnsiTheme", "w:eastAsiaTheme", "w:cstheme"):
        rfonts.attrib.pop(qn(attr), None)

enc = sec.header.paragraphs[0]
enc.alignment = WD_ALIGN_PARAGRAPH.RIGHT
escribir(enc, "NovaBook · Pruebas unitarias", tam=9, color=GRIS)
pie = sec.footer.paragraphs[0]
pie.alignment = WD_ALIGN_PARAGRAPH.CENTER
escribir(pie, "Página ", tam=9, color=GRIS)
campo(pie, "PAGE")

for b in B:
    tipo = b[0]
    if tipo == "titulo":
        pr = doc.add_paragraph()
        pr.alignment = WD_ALIGN_PARAGRAPH.CENTER
        pr.paragraph_format.space_before = Pt(40)
        escribir(pr, b[1], tam=28, color=RGBColor(0x3B, 0x34, 0x86), negrita=True)
    elif tipo == "subtitulo":
        pr = doc.add_paragraph()
        pr.alignment = WD_ALIGN_PARAGRAPH.CENTER
        escribir(pr, b[1], tam=15, color=GRIS)
    elif tipo == "version":
        pr = doc.add_paragraph()
        pr.alignment = WD_ALIGN_PARAGRAPH.CENTER
        pr.paragraph_format.space_after = Pt(18)
        escribir(pr, b[1], tam=10, color=RGBColor(0x77, 0x77, 0x77))
        pr.runs[0].italic = True
    elif tipo == "indice":
        doc.add_heading("Contenido", level=1)
        for c in B:
            if c[0] in ("h1", "h2"):
                pr = doc.add_paragraph()
                pr.paragraph_format.space_after = Pt(0)
                pr.paragraph_format.left_indent = Inches(0 if c[0] == "h1" else 0.35)
                escribir(pr, c[1], tam=10.5 if c[0] == "h1" else 9.5)
        doc.add_page_break()
    elif tipo == "h1":
        doc.add_heading(b[1], level=1)
    elif tipo == "h2":
        doc.add_heading(b[1], level=2)
    elif tipo == "p":
        pr = doc.add_paragraph()
        escribir(pr, b[1], tam=10 if b[2] == "nota" else None, color=GRIS if b[2] == "nota" else None)
    elif tipo == "tabla":
        _, encabezados, filas, anchos = b
        t = doc.add_table(rows=1, cols=len(encabezados))
        t.style = "Table Grid"
        t.alignment = WD_TABLE_ALIGNMENT.CENTER
        t.autofit = False
        fila_enc = t.rows[0]
        trPr = fila_enc._tr.get_or_add_trPr()
        repetir = OxmlElement("w:tblHeader")
        repetir.set(qn("w:val"), "true")
        trPr.append(repetir)
        for i, titulo in enumerate(encabezados):
            celda = fila_enc.cells[i]
            sombrear(celda, INDIGO)
            celda.paragraphs[0].text = ""
            escribir(celda.paragraphs[0], titulo, tam=9.5, color=RGBColor(0xFF, 0xFF, 0xFF), negrita=True)
        for fila in filas:
            nueva = t.add_row()
            sin_partir = OxmlElement("w:cantSplit")   # el renglón no se parte entre páginas
            sin_partir.set(qn("w:val"), "true")
            nueva._tr.get_or_add_trPr().append(sin_partir)
            celdas = nueva.cells
            for i, valor in enumerate(fila):
                txt, clase = (valor if isinstance(valor, tuple) else (valor, ""))
                par = celdas[i].paragraphs[0]
                if clase == "etiqueta":
                    sombrear(celdas[i], "F2F2F2")
                if clase == "mono":
                    escribir(par, f"`{txt}`" if txt else "")
                else:
                    escribir(par, txt, tam=9.5, color=VERDE if clase == "ok" else ROJO if clase == "falla" else None,
                             negrita=clase in ("ok", "falla", "etiqueta"))
        for fila in t.rows:
            for i, ancho in enumerate(anchos):
                fila.cells[i].width = Inches(ancho)
        doc.add_paragraph().paragraph_format.space_after = Pt(2)

SALIDA_DOCX.parent.mkdir(parents=True, exist_ok=True)
doc.save(SALIDA_DOCX)

# ---------------------------------------------------------------------------
# Salida .html (Edge la imprime a PDF)
# ---------------------------------------------------------------------------
def a_html(t):
    s = html.escape(t, quote=False)
    s = re.sub(r"`([^`]+)`", r"<span class='mono'>\1</span>", s)
    return re.sub(r"\*\*([^*]+)\*\*", r"<b>\1</b>", s)


CSS = """
@page { size: letter landscape; margin: 16mm 15mm 16mm 15mm; }
body { font-family: 'Times New Roman', serif; font-size: 10.5pt; color: #1a1a1a; }
h1 { font-size: 19pt; color: #2E74B5; font-weight: normal; margin: 16pt 0 5pt; break-after: avoid; }
h2 { font-size: 14pt; color: #2E74B5; font-weight: normal; margin: 12pt 0 3pt; break-after: avoid; }
h2 + p { break-after: avoid; }
table { break-before: avoid; }
p { margin: 0 0 6pt; }
.titulo { font-size: 28pt; color: #3B3486; font-weight: bold; text-align: center; margin-top: 50pt; }
.subtitulo { font-size: 15pt; color: #555; text-align: center; }
.version { font-size: 10pt; color: #777; font-style: italic; text-align: center; margin-bottom: 22pt; }
.nota { font-size: 9.5pt; color: #555; }
.indice p { margin: 0; }
.indice .n2 { margin-left: 18pt; font-size: 9.5pt; }
table { border-collapse: collapse; width: 100%; margin: 3pt 0 10pt; }
thead { display: table-header-group; }
tr { break-inside: avoid; }
th { background: #3B3486; color: #fff; text-align: left; padding: 3pt 5pt; border: 1px solid #3B3486; font-size: 9.5pt; }
td { border: 1px solid #666; padding: 3pt 5pt; vertical-align: top; font-size: 9.5pt; }
.ok { color: #1E7B34; font-weight: bold; }
.falla { color: #B00020; font-weight: bold; }
.mono { font-family: 'Courier New', monospace; font-size: 9pt; }
.etiqueta { background: #F2F2F2; font-weight: bold; }
.salto { break-after: page; }
"""
out = [f"<!doctype html><html lang='es'><head><meta charset='utf-8'><title>NovaBook — Pruebas unitarias</title><style>{CSS}</style></head><body>"]
for b in B:
    tipo = b[0]
    if tipo in ("titulo", "subtitulo", "version"):
        out.append(f"<p class='{tipo}'>{a_html(b[1])}</p>")
    elif tipo == "indice":
        out.append("<div class='indice salto'><h1>Contenido</h1>")
        for c in B:
            if c[0] in ("h1", "h2"):
                out.append(f"<p class='{'n1' if c[0] == 'h1' else 'n2'}'>{a_html(c[1])}</p>")
        out.append("</div>")
    elif tipo in ("h1", "h2"):
        out.append(f"<{tipo}>{a_html(b[1])}</{tipo}>")
    elif tipo == "p":
        out.append(f"<p class='{b[2]}'>{a_html(b[1])}</p>")
    elif tipo == "tabla":
        _, encabezados, filas, anchos = b
        suma = sum(anchos)
        out.append("<table><colgroup>" + "".join(f"<col style='width:{a / suma * 100:.1f}%'>" for a in anchos) + "</colgroup>")
        out.append("<thead><tr>" + "".join(f"<th>{a_html(t)}</th>" for t in encabezados) + "</tr></thead><tbody>")
        for fila in filas:
            celdas = []
            for valor in fila:
                txt, clase = (valor if isinstance(valor, tuple) else (valor, ""))
                celdas.append(f"<td class='{clase}'>{a_html(txt)}</td>")
            out.append("<tr>" + "".join(celdas) + "</tr>")
        out.append("</tbody></table>")
out.append("</body></html>")
SALIDA_HTML.write_text("\n".join(out), encoding="utf-8")
print(f"{total} pruebas de casos de uso ({aprobadas} aprobadas, {fallidas} fallidas); {len(internas)} internas")
print(f"docx: {SALIDA_DOCX}")
