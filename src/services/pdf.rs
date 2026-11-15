//! Acta de Devolución a Proveedor en PDF (diseño docs/disenos/21.png).
//!
//! Se genera "a mano" (PDF 1.4) para no agregar dependencias: Helvetica y
//! Helvetica-Bold estándar con WinAnsiEncoding (admite acentos y ñ), líneas
//! y varias páginas cuando el detalle no cabe en una.

use crate::productos::model::{Devolucion, TipoProducto};

pub struct PdfService;

// Hoja carta en puntos.
const ANCHO: f32 = 612.0;
const ALTO: f32 = 792.0;
const MARGEN: f32 = 50.0;
const DERECHA: f32 = ANCHO - MARGEN;

// Columnas de la tabla (diseño 21: EAN · TIPO · TÍTULO / DESCRIPCIÓN · CANT. · MOTIVO).
const COL_EAN: f32 = MARGEN;
const COL_TIPO: f32 = 150.0;
const COL_TITULO: f32 = 205.0;
const COL_CANT_DER: f32 = 425.0;
const COL_MOTIVO: f32 = 435.0;
const ALTO_FILA: f32 = 32.0;

impl PdfService {
    pub fn generar_pdf_devolucion(devolucion: &Devolucion, tipo_producto: TipoProducto) -> Vec<u8> {
        let mut doc = Documento::default();
        let tipo = tipo_producto.as_str();
        let proveedor = devolucion.nombre_proveedor.as_deref().unwrap_or("Proveedor");
        let rfc = devolucion.proveedor_rfc.as_deref().unwrap_or("N/D");
        let lote = format!("#{}", devolucion.folio);
        let fecha = fecha_larga(&devolucion.fecha);

        // --- Encabezado ---------------------------------------------------
        let mut y = ALTO - 70.0;
        doc.texto(MARGEN, y, Fuente::Negrita, 16.0, 0.08, "Acta de Devolución a Proveedor");
        y -= 22.0;
        doc.linea(MARGEN, y, DERECHA, y, 0.5, 0.85);
        y -= 20.0;
        doc.texto(MARGEN, y, Fuente::Negrita, 9.5, 0.1, "Proveedor:");
        doc.texto(MARGEN + ancho("Proveedor: ", Fuente::Negrita, 9.5), y, Fuente::Normal, 9.5, 0.1, proveedor);
        let lote_txt = format!("Lote: {}", lote);
        doc.texto_der(DERECHA, y, Fuente::Negrita, 9.5, 0.1, &lote_txt);
        y -= 13.0;
        doc.texto(MARGEN, y, Fuente::Normal, 8.0, 0.45, &format!("RFC: {}", rfc));
        doc.texto_der(DERECHA, y, Fuente::Normal, 8.0, 0.45, &fecha);
        y -= 14.0;
        doc.linea(MARGEN, y, DERECHA, y, 0.5, 0.85);

        // --- Tabla ----------------------------------------------------------
        y -= 24.0;
        let encabezado = |doc: &mut Documento, y: f32| {
            doc.texto(COL_EAN + 4.0, y, Fuente::Negrita, 7.5, 0.35, "EAN");
            doc.texto(COL_TIPO, y, Fuente::Negrita, 7.5, 0.35, "TIPO");
            doc.texto(COL_TITULO, y, Fuente::Negrita, 7.5, 0.35, "TÍTULO / DESCRIPCIÓN");
            doc.texto_der(COL_CANT_DER, y, Fuente::Negrita, 7.5, 0.35, "CANT.");
            doc.texto(COL_MOTIVO, y, Fuente::Negrita, 7.5, 0.35, "MOTIVO");
            doc.linea(MARGEN, y - 7.0, DERECHA, y - 7.0, 1.2, 0.1);
        };
        encabezado(&mut doc, y);
        y -= 7.0;

        for item in &devolucion.items {
            if y - ALTO_FILA < 150.0 {
                doc.nueva_pagina();
                y = ALTO - 70.0;
                encabezado(&mut doc, y);
                y -= 7.0;
            }
            let base = y - 18.0;
            doc.texto(COL_EAN + 4.0, base, Fuente::Normal, 9.0, 0.1, &item.codigo_ean.to_string());
            doc.texto(COL_TIPO, base, Fuente::Normal, 9.0, 0.1, tipo);
            doc.texto(COL_TITULO, base, Fuente::Negrita, 9.0, 0.1, &recortar(&item.titulo, 200.0, Fuente::Negrita, 9.0));
            if let Some(sku) = item.sku {
                doc.texto(COL_TITULO, base - 10.0, Fuente::Normal, 7.5, 0.45, &format!("SKU {}", sku));
            }
            doc.texto_der(COL_CANT_DER, base, Fuente::Normal, 9.0, 0.1, &item.cantidad.to_string());
            let motivo = item.motivo.as_deref().unwrap_or("N/D");
            doc.texto(COL_MOTIVO, base, Fuente::Normal, 9.0, 0.1, &recortar(motivo, DERECHA - COL_MOTIVO, Fuente::Normal, 9.0));
            y -= ALTO_FILA;
            doc.linea(MARGEN, y, DERECHA, y, 0.5, 0.88);
        }

        // --- Total y firmas ---------------------------------------------------
        y -= 18.0;
        doc.texto_der(DERECHA, y, Fuente::Negrita, 9.5, 0.1,
                      &format!("Total de piezas a devolver: {}", devolucion.total_piezas));

        y -= 70.0;
        let mitad = (MARGEN + DERECHA) / 2.0;
        doc.linea(MARGEN, y, mitad - 16.0, y, 0.6, 0.35);
        doc.linea(mitad + 16.0, y, DERECHA, y, 0.6, 0.35);
        doc.texto_centro((MARGEN + mitad - 16.0) / 2.0, y - 13.0, Fuente::Normal, 8.5, 0.2, "Entrega (Bodega)");
        doc.texto_centro((mitad + 16.0 + DERECHA) / 2.0, y - 13.0, Fuente::Normal, 8.5, 0.2, "Recibe (Proveedor)");

        // Trazabilidad (no aparece en el diseño, pero da validez al acta).
        let solicito = devolucion.vendedor_nombre.as_deref().unwrap_or("N/D");
        let autorizo = devolucion.autorizado_por.as_deref().unwrap_or("N/D");
        doc.texto(MARGEN, 40.0, Fuente::Normal, 7.0, 0.55,
                  &format!("Solicitó: {} · Autorizó: {} · Estado: {}", solicito, autorizo, devolucion.estado));

        doc.construir()
    }
}

// ============================================================================
// Mini generador de PDF
// ============================================================================

#[derive(Clone, Copy)]
enum Fuente {
    Normal,
    Negrita,
}

#[derive(Default)]
struct Documento {
    paginas: Vec<String>, // flujo de contenido de cada página
}

impl Documento {
    fn actual(&mut self) -> &mut String {
        if self.paginas.is_empty() {
            self.paginas.push(String::new());
        }
        self.paginas.last_mut().unwrap()
    }

    fn nueva_pagina(&mut self) {
        self.paginas.push(String::new());
    }

    /// Texto en (x, y) con tono de gris (0 = negro, 1 = blanco).
    fn texto(&mut self, x: f32, y: f32, f: Fuente, tam: f32, gris: f32, s: &str) {
        let nombre = match f { Fuente::Normal => "F1", Fuente::Negrita => "F2" };
        let op = format!("BT {g:.2} g /{nombre} {tam:.1} Tf {x:.1} {y:.1} Td ({}) Tj ET\n", escapar(s), g = gris);
        self.actual().push_str(&op);
    }

    fn texto_der(&mut self, x_der: f32, y: f32, f: Fuente, tam: f32, gris: f32, s: &str) {
        self.texto(x_der - ancho(s, f, tam), y, f, tam, gris, s);
    }

    fn texto_centro(&mut self, x_centro: f32, y: f32, f: Fuente, tam: f32, gris: f32, s: &str) {
        self.texto(x_centro - ancho(s, f, tam) / 2.0, y, f, tam, gris, s);
    }

    fn linea(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, grosor: f32, gris: f32) {
        let op = format!("{gris:.2} G {grosor:.2} w {x1:.1} {y1:.1} m {x2:.1} {y2:.1} l S\n");
        self.actual().push_str(&op);
    }

    fn construir(mut self) -> Vec<u8> {
        if self.paginas.is_empty() {
            self.nueva_pagina();
        }
        let n = self.paginas.len();
        // Objetos: 1 catálogo, 2 páginas, 3 Helvetica, 4 Helvetica-Bold,
        // y por página i: (5 + 2i) página, (6 + 2i) contenido.
        let mut objetos: Vec<Vec<u8>> = Vec::new();
        objetos.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
        let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", 5 + 2 * i)).collect();
        objetos.push(format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), n).into_bytes());
        objetos.push(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_vec());
        objetos.push(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>".to_vec());
        for (i, contenido) in self.paginas.iter().enumerate() {
            objetos.push(format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {ANCHO} {ALTO}] /Contents {} 0 R \
                 /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> >>",
                6 + 2 * i
            ).into_bytes());
            let bytes = a_winansi(contenido);
            let mut obj = format!("<< /Length {} >>\nstream\n", bytes.len()).into_bytes();
            obj.extend_from_slice(&bytes);
            obj.extend_from_slice(b"\nendstream");
            objetos.push(obj);
        }

        let mut pdf: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
        let mut offsets = Vec::with_capacity(objetos.len());
        for (i, obj) in objetos.iter().enumerate() {
            offsets.push(pdf.len());
            pdf.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
            pdf.extend_from_slice(obj);
            pdf.extend_from_slice(b"\nendobj\n");
        }
        let xref = pdf.len();
        pdf.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objetos.len() + 1).as_bytes());
        for off in offsets {
            pdf.extend_from_slice(format!("{:010} 00000 n \n", off).as_bytes());
        }
        pdf.extend_from_slice(
            format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n", objetos.len() + 1, xref).as_bytes(),
        );
        pdf
    }
}

/// Escapa los caracteres especiales de una cadena literal de PDF.
fn escapar(s: &str) -> String {
    s.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)")
}

/// Convierte el flujo a WinAnsi (Latin-1 + algunos signos); lo demás → '?'.
fn a_winansi(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| match c {
            // Latin-1 (incluye á é í ó ú ñ ü ·) coincide con WinAnsi.
            '\u{0000}'..='\u{007F}' | '\u{00A0}'..='\u{00FF}' => c as u32 as u8,
            '—' => 0x97,
            '–' => 0x96,
            '“' => 0x93,
            '”' => 0x94,
            '‘' => 0x91,
            '’' => 0x92,
            '€' => 0x80,
            _ => b'?',
        })
        .collect()
}

/// Ancho aproximado de un texto en Helvetica (unidades de 1/1000 del tamaño).
/// Basta para alinear a la derecha y recortar títulos.
fn ancho(s: &str, f: Fuente, tam: f32) -> f32 {
    let base: f32 = s
        .chars()
        .map(|c| match c {
            '0'..='9' | '#' | '$' | '_' => 556.0,
            ' ' | '.' | ',' | ':' | ';' | 'i' | 'l' | 'j' | '\'' | '|' => 278.0,
            '-' | '(' | ')' | 'r' | 't' | 'f' | 'I' => 333.0,
            'm' | 'w' | 'M' | 'W' => 833.0,
            'A'..='Z' | 'Á' | 'É' | 'Í' | 'Ó' | 'Ú' | 'Ñ' => 667.0,
            _ => 556.0,
        })
        .sum();
    let factor = match f { Fuente::Normal => 1.0, Fuente::Negrita => 1.06 };
    base * factor * tam / 1000.0
}

fn recortar(s: &str, max: f32, f: Fuente, tam: f32) -> String {
    if ancho(s, f, tam) <= max {
        return s.to_string();
    }
    let mut r: String = s.to_string();
    while !r.is_empty() && ancho(&format!("{}...", r), f, tam) > max {
        r.pop();
    }
    format!("{}...", r.trim_end())
}

/// "2026-10-14" → "14 de octubre de 2026"
fn fecha_larga(iso: &str) -> String {
    const MESES: [&str; 12] = [
        "enero", "febrero", "marzo", "abril", "mayo", "junio",
        "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre",
    ];
    let p: Vec<&str> = iso.get(..10).unwrap_or(iso).split('-').collect();
    match (p.first(), p.get(1).and_then(|m| m.parse::<usize>().ok()), p.get(2).and_then(|d| d.parse::<u32>().ok())) {
        (Some(a), Some(m), Some(d)) if (1..=12).contains(&m) => format!("{} de {} de {}", d, MESES[m - 1], a),
        _ => iso.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::productos::model::ItemDevolucion;

    fn devolucion(n_items: usize) -> Devolucion {
        Devolucion {
            id_devolucion: 4,
            folio: "DEV-2026-4".into(),
            solicitante_rol: Some("Vendedor".into()),
            fecha: "2026-10-14".into(),
            total_piezas: n_items as i32,
            estado: "APROBADO".into(),
            autorizado_por: Some("Marco Pérez".into()),
            id_usuarios: 628777129,
            vendedor_nombre: Some("Aldo Soto".into()),
            id_proveedor: 2,
            nombre_proveedor: Some("Editorial Televisa".into()),
            proveedor_rfc: Some("ETE730508GH4".into()),
            proveedor_telefono: None,
            proveedor_correo: None,
            proveedor_direccion: None,
            proveedor_contacto: None,
            tipo_producto: TipoProducto::Libro,
            observaciones_rechazo: None,
            items: (0..n_items)
                .map(|i| ItemDevolucion {
                    codigo_ean: 9780141439518 + i as i64,
                    id_motivo: 1,
                    sku: Some(884102),
                    titulo: "Orgullo y prejuicio (edición de colección)".into(),
                    cantidad: 1,
                    motivo: Some("Defecto de Imprenta".into()),
                })
                .collect(),
        }
    }

    #[test]
    fn genera_pdf_valido_con_acentos() {
        let pdf = PdfService::generar_pdf_devolucion(&devolucion(2), TipoProducto::Libro);
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(pdf.ends_with(b"%%EOF\n"));
        // "Devolución" en WinAnsi: la ó es el byte 0xF3.
        assert!(pdf.windows(10).any(|w| w == b"Devoluci\xF3n"));
        assert!(pdf.windows(13).any(|w| w == b"#DEV-2026-4) "));
        assert!(pdf.windows(9).any(|w| w == b"/Count 1 "));
    }

    #[test]
    fn varias_paginas_si_no_cabe() {
        let pdf = PdfService::generar_pdf_devolucion(&devolucion(40), TipoProducto::Libro);
        let s = String::from_utf8_lossy(&pdf);
        assert!(!s.contains("/Count 1 "), "40 partidas no caben en una hoja");
    }

    #[test]
    fn xref_apunta_a_los_objetos() {
        // Se trabaja en bytes: los acentos WinAnsi no son UTF-8 válido.
        let pdf = PdfService::generar_pdf_devolucion(&devolucion(3), TipoProducto::Revista);
        let pos = pdf.windows(10).rposition(|w| w == b"startxref\n").unwrap() + 10;
        let fin = pos + pdf[pos..].iter().position(|&b| b == b'\n').unwrap();
        let start: usize = std::str::from_utf8(&pdf[pos..fin]).unwrap().parse().unwrap();
        assert!(pdf[start..].starts_with(b"xref"));
        // Cada offset de la tabla debe caer en "N 0 obj".
        let tabla = std::str::from_utf8(&pdf[start..]).unwrap();
        let mut revisados = 0;
        for (i, linea) in tabla.lines().skip(3).take_while(|l| l.ends_with(" n ")).enumerate() {
            let off: usize = linea[..10].parse().unwrap();
            assert!(pdf[off..].starts_with(format!("{} 0 obj", i + 1).as_bytes()), "objeto {}", i + 1);
            revisados += 1;
        }
        assert_eq!(revisados, 6, "catálogo, páginas, 2 fuentes, 1 página y su contenido");
    }

    #[test]
    fn fecha_en_espanol() {
        assert_eq!(fecha_larga("2026-10-14"), "14 de octubre de 2026");
        assert_eq!(fecha_larga("2026-01-05 10:00:00"), "5 de enero de 2026");
    }
}
