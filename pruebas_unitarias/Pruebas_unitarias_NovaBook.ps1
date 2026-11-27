# =============================================================================
#  NovaBook - Ejecucion y reporte de las pruebas unitarias Primera Revision
# =============================================================================
#
#  Uso (o doble clic en Ejecutar_pruebas_unitarias.bat, que muestra un menu):
#    .\Pruebas_unitarias_NovaBook.ps1 -Opcion 1   # Revision segun cronograma antes de cambios (CU-01 a CU-14)
#    .\Pruebas_unitarias_NovaBook.ps1 -Opcion 2   # Hito segun cronograma antes de cambios     (CU-01 a CU-19)
#    .\Pruebas_unitarias_NovaBook.ps1 -Opcion 3   # Revision segun cronograma actualizado      (CU-01 a CU-25)
#    .\Pruebas_unitarias_NovaBook.ps1 -Opcion 4   # Hito segun cronograma actualizado          (CU-01 a CU-32)
#  Otras formas:
#    .\Pruebas_unitarias_NovaBook.ps1                     # lo que vence hasta el 27/11/2026 (CU-01 a CU-14)
#    .\Pruebas_unitarias_NovaBook.ps1 -Hasta 2026-11-30   # lo que vence hasta otra fecha
#    .\Pruebas_unitarias_NovaBook.ps1 -Cronograma         # los 19 casos de uso del cronograma
#    .\Pruebas_unitarias_NovaBook.ps1 -Todos              # los 32 (cronograma + agregados)
#    .\Pruebas_unitarias_NovaBook.ps1 -Solo 13,14         # solo esos casos de uso
#    .\Pruebas_unitarias_NovaBook.ps1 -LimiteMin 20       # tiempo maximo de la corrida (15 min por omision)
#    .\Pruebas_unitarias_NovaBook.ps1 -Opcion 2 -Proyecto C:\ruta\API-Novabook  # si no encuentra el repo solo
#
#  Vive en el repo (API-Novabook\pruebas_unitarias) y tambien sirve copiado en
#  NovaBook-v2\03_Pruebas_unitarias: busca el repo en su carpeta y en la de arriba.
#
#  Todas las pruebas seleccionadas se corren en UNA sola llamada a cargo test:
#  la base de pruebas se crea una vez (recrearla muchas veces seguidas llego a
#  atorar al MariaDB de XAMPP). Si la corrida pasa del limite de tiempo, el
#  script la detiene y reporta esas pruebas como "No ejecutadas".
#
#  Requisitos: Rust (cargo), XAMPP (MariaDB en el puerto 3306; si esta apagado
#  el script lo enciende) y el codigo de API-Novabook con la carpeta src/pruebas.
#  Las pruebas usan su propia base (Libreria_pruebas), que se crea sola en cada
#  corrida: la base Libreria de la app no se toca.
#
#  Salida: carpeta Reportes\<fecha_hora>_opcionN\ junto al script, con
#    Reporte_pruebas_unitarias.txt   reporte completo en texto
#    Reporte_pruebas_unitarias.html  el mismo reporte para ver o imprimir
#    resultados.csv                  una fila por prueba (para Excel)
#    salida_cargo_test.log           salida original de cargo test (evidencia)
# =============================================================================
param(
    # 1 Revision segun cronograma antes de cambios (CU-01 a CU-14)
    # 2 Hito segun cronograma antes de cambios     (CU-01 a CU-19)
    # 3 Revision segun cronograma actualizado      (CU-01 a CU-25)
    # 4 Hito segun cronograma actualizado          (CU-01 a CU-32)
    # 0 = no usar opcion; se usan -Hasta, -Cronograma, -Todos o -Solo
    [ValidateSet(0, 1, 2, 3, 4)]
    [int]$Opcion = 0,
    [string]$Hasta = "2026-11-27",
    [switch]$Cronograma,
    [switch]$Todos,
    [int[]]$Solo = @(),
    [int]$LimiteMin = 15,
    # Carpeta del proyecto (la que contiene API-Novabook). Por omision, la del
    # script o la de arriba (03_Pruebas_unitarias); si no, C:\Proyectos\NovaBook-v2.
    [string]$Proyecto = ""
)

$ErrorActionPreference = "Stop"
# Donde esta la API: se acepta una carpeta que CONTIENE API-Novabook o la que ES
# el repo de la API (tiene src\pruebas). Se busca en -Proyecto, en la carpeta
# del script, en la de arriba y en C:\Proyectos\NovaBook-v2. Asi funciona en
# NovaBook-v2\03_Pruebas_unitarias y dentro del repo (API-Novabook\pruebas_unitarias).
$candidatos = @()
if ($Proyecto) { $candidatos += $Proyecto }
$candidatos += $PSScriptRoot, (Split-Path $PSScriptRoot -Parent), "C:\Proyectos\NovaBook-v2"
$Api = $null
foreach ($c in ($candidatos | Where-Object { $_ })) {
    if (Test-Path (Join-Path $c "API-Novabook\src\pruebas")) { $Api = Join-Path $c "API-Novabook"; break }
    if (Test-Path (Join-Path $c "src\pruebas")) { $Api = $c; break }
}
if (-not $Api) {
    Write-Host "No se encontro el repo API-Novabook con la carpeta src\pruebas." -ForegroundColor Red
    Write-Host "Haz git pull de API-Novabook (rama con las pruebas) y corre el script desde API-Novabook\pruebas_unitarias," -ForegroundColor Red
    Write-Host "o indica donde esta el repo:" -ForegroundColor Red
    Write-Host "   Ejecutar_pruebas_unitarias.bat -Opcion 2 -Proyecto C:\ruta\a\API-Novabook" -ForegroundColor Yellow
    exit 2
}
$Api = (Resolve-Path $Api).Path
$Raiz = Split-Path $Api -Parent
$Pruebas = Join-Path $Api "src\pruebas"
$Cargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"
$Rustc = Join-Path $env:USERPROFILE ".cargo\bin\rustc.exe"
$MySqlBin = "C:\xampp\mysql\bin"

# -----------------------------------------------------------------------------
# Casos de uso: numero, tarea del cronograma, nombre, RF, fechas y origen
# (cronograma NovaBook_V5.pdf, fase de Transicion; los agregados no tienen fecha)
# -----------------------------------------------------------------------------
function Nuevo-Caso($n, $tarea, $nombre, $rf, $inicio, $fin, $origen) {
    [pscustomobject]@{ Num = $n; Tarea = $tarea; Nombre = $nombre; RF = $rf
        Inicio = $(if ($inicio) { [datetime]$inicio } else { $null })
        Fin = $(if ($fin) { [datetime]$fin } else { $null }); Origen = $origen }
}
$Casos = @(
    Nuevo-Caso 1  83  "Login"                                       "RF15" "2026-11-21" "2026-11-23" "Cronograma"
    Nuevo-Caso 2  84  "CRUD de Usuarios"                            "RF01" "2026-11-21" "2026-11-23" "Cronograma"
    Nuevo-Caso 3  85  "Catalogo de Autores"                         "RF18" "2026-11-22" "2026-11-23" "Cronograma"
    Nuevo-Caso 4  86  "Catalogo de Generos"                         "RF19" "2026-11-22" "2026-11-23" "Cronograma"
    Nuevo-Caso 5  87  "Catalogo de Muebles y Estanteria"            "RF21" "2026-11-23" "2026-11-23" "Cronograma"
    Nuevo-Caso 6  88  "Motivos de devolucion"                       "RF20" "2026-11-23" "2026-11-23" "Cronograma"
    Nuevo-Caso 7  89  "Registrar Libros"                            "RF05" "2026-11-24" "2026-11-24" "Cronograma"
    Nuevo-Caso 8  90  "Registrar Revistas"                          "RF06" "2026-11-24" "2026-11-24" "Cronograma"
    Nuevo-Caso 9  91  "Registrar compra a proveedor"                "RF17" "2026-11-25" "2026-11-25" "Cronograma"
    Nuevo-Caso 10 92  "Consultar existencias"                       "RF10" "2026-11-25" "2026-11-25" "Cronograma"
    Nuevo-Caso 11 93  "Movimiento Bodega-Tienda Libros"             "RF07" "2026-11-26" "2026-11-26" "Cronograma"
    Nuevo-Caso 12 94  "Movimiento Bodega-Tienda Revistas"           "RF08" "2026-11-26" "2026-11-26" "Cronograma"
    Nuevo-Caso 13 95  "Movimiento Tienda-Bodega Libros"             "RF11" "2026-11-27" "2026-11-27" "Cronograma"
    Nuevo-Caso 14 96  "Movimiento Tienda-Bodega Revistas"           "RF12" "2026-11-27" "2026-11-27" "Cronograma"
    Nuevo-Caso 15 97  "Ventas"                                      "RF09" "2026-11-28" "2026-11-30" "Cronograma"
    Nuevo-Caso 16 98  "Aprobar devoluciones"                        "RF04" "2026-11-28" "2026-11-30" "Cronograma"
    Nuevo-Caso 17 99  "PDF de devolucion a Proveedor Libros"        "RF13" "2026-11-29" "2026-11-30" "Cronograma"
    Nuevo-Caso 18 100 "PDF de devolucion a Proveedor Revistas"      "RF14" "2026-11-29" "2026-11-30" "Cronograma"
    Nuevo-Caso 19 101 "Consultar movimiento diario"                 "RF03" "2026-11-30" "2026-11-30" "Cronograma"
    Nuevo-Caso 20 $null "Gestionar usuarios (Jefe de Departamento)" "RF16" $null $null "Agregado"
    Nuevo-Caso 21 $null "Consultar historial de devoluciones"       "RF02" $null $null "Agregado"
    Nuevo-Caso 22 $null "Devolucion de cliente"                     "RF22" $null $null "Agregado"
    Nuevo-Caso 23 $null "Levantar inventario mensual"               "RF23" $null $null "Agregado"
    Nuevo-Caso 24 $null "Revisar inventario mensual"                "RF24" $null $null "Agregado"
    Nuevo-Caso 25 $null "Registrar solicitudes de editorial"        "RF25" $null $null "Agregado"
    Nuevo-Caso 26 $null "Consultar mis devoluciones"                "RF26" $null $null "Agregado"
    Nuevo-Caso 27 $null "Gestionar proveedores y editoriales"       "RF27" $null $null "Agregado"
    Nuevo-Caso 28 $null "Cierre de sesion automatico"               "RF28" $null $null "Agregado"
    Nuevo-Caso 29 $null "Registrar incidencia de compra"            "RF29" $null $null "Agregado"
    Nuevo-Caso 30 $null "Recibir requisicion de salida"             "RF30" $null $null "Agregado"
    Nuevo-Caso 31 $null "Registrar devolucion a proveedor"          "-"    $null $null "Agregado"
    Nuevo-Caso 32 $null "Tickets de venta (hito 2, CR-014)"         "-"    $null $null "Agregado"
)

# Defectos ya conocidos (Pruebas unitarias NovaBook, seccion 4): explican por
# que fallan esas pruebas a proposito.
$Defectos = @{
    "pu_13_07_requisiciones_simultaneas" = "D-02: bloqueo mutuo de MariaDB (error 1213) cuando una bajada bodega->piso y un regreso piso->bodega se registran al mismo tiempo; la API responde 500. Correccion propuesta: reintentar la transaccion (hasta 3 veces) o bloquear los muebles siempre en el mismo orden."
    "pu_17_06_tipo_de_acta_coincide"     = "D-01: el acta PDF no valida que la ruta /pdf/libros|revistas coincida con el tipo de la devolucion (responde 200). Correccion propuesta: responder 400 en DevolucionService::generar_pdf_devolucion."
    "pu_18_05_tipo_de_acta_coincide"     = "D-01: el acta PDF no valida que la ruta /pdf/libros|revistas coincida con el tipo de la devolucion (responde 200). Correccion propuesta: responder 400 en DevolucionService::generar_pdf_devolucion."
}

# -----------------------------------------------------------------------------
# Que casos de uso se corren
# -----------------------------------------------------------------------------
$fechaHasta = [datetime]$Hasta
$Opciones = @{
    1 = @{ Nombre = "Opcion 1 - Revision segun cronograma antes de cambios"; Hasta = 14
           Detalle = "CU-01 a CU-14: de Login a Movimiento Tienda-Bodega Revistas (tareas 83 a 96 del cronograma)" }
    2 = @{ Nombre = "Opcion 2 - Hito segun cronograma antes de cambios"; Hasta = 19
           Detalle = "CU-01 a CU-19: las 19 pruebas unitarias del cronograma (tareas 83 a 101)" }
    3 = @{ Nombre = "Opcion 3 - Revision segun cronograma actualizado"; Hasta = 25
           Detalle = "CU-01 a CU-25: las 19 del cronograma y CU-20 a CU-25 de los casos de uso nuevos" }
    4 = @{ Nombre = "Opcion 4 - Hito segun cronograma actualizado"; Hasta = 32
           Detalle = "CU-01 a CU-32: todos los casos de uso del cronograma actualizado" }
}
$NombreOpcion = ""
if ($Opcion -gt 0) {
    $o = $Opciones[$Opcion]
    $Seleccion = $Casos | Where-Object { $_.Num -le $o.Hasta }
    $NombreOpcion = $o.Nombre
    $Alcance = "$($o.Nombre): $($o.Detalle)"
} elseif ($Solo.Count -gt 0) {
    $Seleccion = $Casos | Where-Object { $Solo -contains $_.Num }
    $Alcance = "Casos de uso indicados: " + (($Seleccion | ForEach-Object { "CU-{0:D2}" -f $_.Num }) -join ", ")
} elseif ($Todos) {
    $Seleccion = $Casos
    $Alcance = "Los 32 casos de uso (19 del cronograma y 13 agregados del diagrama de casos de uso v4)"
} elseif ($Cronograma) {
    $Seleccion = $Casos | Where-Object { $_.Origen -eq "Cronograma" }
    $Alcance = "Los 19 casos de uso del cronograma (tareas 83 a 101)"
} else {
    $Seleccion = $Casos | Where-Object { $_.Fin -and $_.Fin -le $fechaHasta }
    $Alcance = "Pruebas del cronograma que vencen hasta el {0:dd/MM/yyyy} (tareas {1} a {2})" -f $fechaHasta,
        ($Seleccion | Select-Object -First 1).Tarea, ($Seleccion | Select-Object -Last 1).Tarea
}
if (-not $Seleccion) { throw "No hay casos de uso que correr con esos parametros." }

# -----------------------------------------------------------------------------
# Utilidades
# -----------------------------------------------------------------------------
function Write-Titulo($t) { Write-Host ""; Write-Host ("=" * 78) -ForegroundColor DarkCyan; Write-Host " $t" -ForegroundColor Cyan; Write-Host ("=" * 78) -ForegroundColor DarkCyan }
function Test-Puerto([int]$p) {
    try { $c = New-Object Net.Sockets.TcpClient; $c.Connect("127.0.0.1", $p); $c.Close(); return $true } catch { return $false }
}
function Get-Mensaje-Falla([string[]]$lineas, [string]$ruta) {
    # Bloque "---- <ruta> stdout ----" ... "panicked at ...:" <mensaje> "note: run with"
    $inicio = [Array]::IndexOf($lineas, "---- $ruta stdout ----")
    if ($inicio -lt 0) { return "Fallo (ver salida_cargo_test.log)." }
    $msg = @(); $tomar = $false
    for ($i = $inicio + 1; $i -lt $lineas.Count; $i++) {
        $l = $lineas[$i]
        if ($l -like "---- * stdout ----" -or $l -eq "failures:") { break }
        if ($l -like "note: run with*") { break }
        if ($tomar) { $msg += $l.Trim() }
        if ($l -match "panicked at .*:\s*$") { $tomar = $true }
    }
    $t = (($msg | Where-Object { $_ }) -join " ")
    if (-not $t) { $t = "Fallo (ver salida_cargo_test.log)." }
    if ($t.Length -gt 500) { $t = $t.Substring(0, 500) + "..." }
    return $t
}

$inicioTotal = Get-Date
$Carpeta = Join-Path $PSScriptRoot ("Reportes\" +$inicioTotal.ToString("yyyy-MM-dd_HHmm") + $(if ($Opcion -gt 0) { "_opcion$Opcion" } else { "" }))
New-Item -ItemType Directory -Force $Carpeta | Out-Null
$LogCargo = Join-Path $Carpeta "salida_cargo_test.log"
$utf8 = New-Object Text.UTF8Encoding($true)
[IO.File]::WriteAllText($LogCargo, "", $utf8)

Write-Titulo ("NovaBook - Pruebas unitarias por caso de uso" + $(if ($NombreOpcion) { " - $NombreOpcion" } else { "" }))
Write-Host " Alcance: $Alcance"
Write-Host " Reporte: $Carpeta"

# -----------------------------------------------------------------------------
# 1. Requisitos
# -----------------------------------------------------------------------------
Write-Titulo "1. Revisando requisitos"
if (-not (Test-Path $Cargo)) { throw "No se encontro cargo ($Cargo). Instala Rust con rustup." }
if (-not (Test-Path $Pruebas)) { throw "No se encontro $Pruebas. Abre la rama 'pruebas-unitarias' de API-Novabook." }
Write-Host " [OK] cargo: $(& $Rustc --version)"
Write-Host " [OK] pruebas: $Pruebas"

if (-not (Test-Puerto 3306)) {
    Write-Host " MariaDB esta apagado; encendiendolo (XAMPP)..." -ForegroundColor Yellow
    if (-not (Test-Path "$MySqlBin\mysqld.exe")) { throw "No se encontro MariaDB de XAMPP en $MySqlBin. Enciende MySQL desde el panel de XAMPP." }
    Start-Process -FilePath "$MySqlBin\mysqld.exe" -ArgumentList "--defaults-file=$MySqlBin\my.ini", "--standalone" -WindowStyle Hidden
    for ($i = 0; $i -lt 30 -and -not (Test-Puerto 3306); $i++) { Start-Sleep 1 }
    if (-not (Test-Puerto 3306)) { throw "MariaDB no respondio en el puerto 3306. Enciendelo desde el panel de XAMPP y vuelve a correr el script." }
}
$versionMaria = (& "$MySqlBin\mysql.exe" -u root -N -e "SELECT VERSION();" 2>$null)
Write-Host " [OK] MariaDB $versionMaria en el puerto 3306"

# -----------------------------------------------------------------------------
# 2. Compilacion
# -----------------------------------------------------------------------------
Write-Titulo "2. Compilando la API y las pruebas (cargo test --no-run)"
Push-Location $Api
try { $comp = cmd /c "`"$Cargo`" test --no-run 2>&1" } finally { Pop-Location }
Add-Content -Path $LogCargo -Value ("### Compilacion`n" + ($comp -join "`n")) -Encoding utf8
if ($LASTEXITCODE -ne 0) {
    $comp | Select-Object -Last 25 | ForEach-Object { Write-Host "   $_" -ForegroundColor Red }
    throw "La compilacion fallo; no se corrio ninguna prueba (detalle en $LogCargo)."
}
Write-Host " [OK] Compilacion correcta"

# -----------------------------------------------------------------------------
# 3. Ejecucion por caso de uso
# -----------------------------------------------------------------------------
Write-Titulo "3. Ejecutando las pruebas"
$Resultados = New-Object System.Collections.Generic.List[object]
$Resumen = New-Object System.Collections.Generic.List[object]
$patron = '/// (PU-\d{2}-\d{2}) · (.+)\n/// Entrada: (.+)\n/// Esperado: (.+)\n#\[tokio::test[^\]]*\]\nasync fn (\w+)'

# 3a. Archivo de pruebas de cada caso de uso seleccionado.
$Archivos = @{}
foreach ($cu in $Seleccion) {
    $a = Get-ChildItem $Pruebas -Filter ("cu{0:D2}_*.rs" -f $cu.Num) | Select-Object -First 1
    if ($a) { $Archivos[$cu.Num] = $a }
}
if ($Archivos.Count -eq 0) { throw "Ningun caso de uso seleccionado tiene archivo de pruebas en $Pruebas." }

# 3b. Una sola corrida de cargo test con un filtro por caso de uso.
$filtros = ($Seleccion | Where-Object { $Archivos.ContainsKey($_.Num) } | ForEach-Object { "pruebas::" + $Archivos[$_.Num].BaseName + "::" }) -join " "
$tmpSalida = Join-Path $Carpeta "corrida.tmp"
Write-Host (" cargo test -- {0}" -f $filtros) -ForegroundColor DarkGray
Write-Host (" Corriendo {0} caso(s) de uso (limite {1} min)..." -f $Archivos.Count, $LimiteMin)
$t0 = Get-Date
$proc = Start-Process -FilePath "cmd.exe" -WorkingDirectory $Api -WindowStyle Hidden -PassThru `
    -ArgumentList "/c `"`"$Cargo`" test -- $filtros > `"$tmpSalida`" 2>&1`""
$termino = $proc.WaitForExit($LimiteMin * 60 * 1000)
if (-not $termino) {
    & taskkill.exe /T /F /PID $proc.Id 2>&1 | Out-Null
    Write-Host (" [!] La corrida paso de {0} min y se detuvo; las pruebas sin resultado quedan como 'No ejecutadas'." -f $LimiteMin) -ForegroundColor Yellow
}
$segCorrida = [math]::Round(((Get-Date) - $t0).TotalSeconds, 1)
$salida = @(Get-Content $tmpSalida -Encoding UTF8 -ErrorAction SilentlyContinue)
Remove-Item $tmpSalida -ErrorAction SilentlyContinue
Add-Content -Path $LogCargo -Value ("`n### Pruebas (cargo test -- $filtros)`n" + ($salida -join "`n")) -Encoding utf8
$motivoNoCorrida = if ($termino) { "No aparecio en la salida de cargo test." } else {
    "La corrida se detuvo al pasar $LimiteMin min (posible atoramiento de MariaDB); reinicia MySQL en XAMPP y vuelve a correr." }

# 3c. Resultado de cada prueba, por caso de uso.
foreach ($cu in $Seleccion) {
    $archivo = $Archivos[$cu.Num]
    $etiqueta = "CU-{0:D2} {1}" -f $cu.Num, $cu.Nombre
    if (-not $archivo) {
        Write-Host (" [--] {0,-50} sin archivo de pruebas" -f $etiqueta) -ForegroundColor Yellow
        $Resumen.Add([pscustomobject]@{ Caso = $cu; Archivo = "-"; Total = 0; Ok = 0; Fallas = 0; NoCorridas = 0 })
        continue
    }
    # Pruebas documentadas en el archivo (ID, titulo, entrada y resultado esperado).
    $texto = [IO.File]::ReadAllText($archivo.FullName, [Text.Encoding]::UTF8).Replace("`r`n", "`n")
    $doc = @{}
    foreach ($m in [regex]::Matches($texto, $patron)) {
        $doc[$m.Groups[5].Value] = [pscustomobject]@{ Id = $m.Groups[1].Value; Titulo = $m.Groups[2].Value
            Entrada = $m.Groups[3].Value; Esperado = $m.Groups[4].Value }
    }
    $prefijo = "pruebas::" + $archivo.BaseName + "::"

    $vistos = @{}
    foreach ($l in $salida) {
        if ($l -match '^test (pruebas::\S+::(\w+)) \.\.\. (ok|FAILED|ignored)$' -and $Matches[1].StartsWith($prefijo)) {
            $ruta = $Matches[1]; $fn = $Matches[2]; $estado = $Matches[3]
            $vistos[$fn] = $true
            $d = $doc[$fn]
            $obtenido = switch ($estado) {
                "ok" { "Coincide con lo esperado." }
                "ignored" { "Prueba marcada como ignorada." }
                default { Get-Mensaje-Falla $salida $ruta }
            }
            $Resultados.Add([pscustomobject]@{
                CU = $cu.Num; Id = $(if ($d) { $d.Id } else { $fn }); Funcion = $fn
                Titulo = $(if ($d) { $d.Titulo } else { $fn.Replace("_", " ") })
                Entrada = $(if ($d) { $d.Entrada } else { "-" }); Esperado = $(if ($d) { $d.Esperado } else { "-" })
                Estado = $(switch ($estado) { "ok" { "Aprobada" } "FAILED" { "Fallida" } default { "Ignorada" } })
                Obtenido = $obtenido; Defecto = $(if ($estado -eq "FAILED" -and $Defectos.ContainsKey($fn)) { $Defectos[$fn] } else { "" })
            })
        }
    }
    # Documentadas pero que no aparecieron en la corrida (p. ej. no compilaron o se borraron).
    foreach ($fn in $doc.Keys) {
        if (-not $vistos.ContainsKey($fn)) {
            $d = $doc[$fn]
            $Resultados.Add([pscustomobject]@{ CU = $cu.Num; Id = $d.Id; Funcion = $fn; Titulo = $d.Titulo; Entrada = $d.Entrada
                Esperado = $d.Esperado; Estado = "No ejecutada"; Obtenido = $motivoNoCorrida; Defecto = "" })
        }
    }

    $mias = @($Resultados | Where-Object { $_.CU -eq $cu.Num })
    $ok = @($mias | Where-Object Estado -eq "Aprobada").Count
    $mal = @($mias | Where-Object Estado -eq "Fallida").Count
    $nc = @($mias | Where-Object { $_.Estado -eq "No ejecutada" -or $_.Estado -eq "Ignorada" }).Count
    $Resumen.Add([pscustomobject]@{ Caso = $cu; Archivo = $archivo.Name; Total = $mias.Count; Ok = $ok; Fallas = $mal; NoCorridas = $nc })
    $color = if ($mal -gt 0) { "Red" } elseif ($nc -gt 0) { "Yellow" } else { "Green" }
    Write-Host (" {0,-52} {1,3} pruebas  {2,3} ok  {3,2} fallidas  {4,2} no ejecutadas" -f $etiqueta, $mias.Count, $ok, $mal, $nc) -ForegroundColor $color
    foreach ($f in ($mias | Where-Object Estado -ne "Aprobada")) {
        Write-Host ("     x {0} {1}: {2}" -f $f.Id, $f.Titulo, $f.Estado) -ForegroundColor $color
    }
}
Write-Host (" Tiempo de la corrida de pruebas: {0} s" -f $segCorrida)
$finTotal = Get-Date
$duracion = [math]::Round(($finTotal - $inicioTotal).TotalSeconds)

# -----------------------------------------------------------------------------
# 4. Reportes
# -----------------------------------------------------------------------------
$tot = $Resultados.Count
$totOk = @($Resultados | Where-Object Estado -eq "Aprobada").Count
$totMal = @($Resultados | Where-Object Estado -eq "Fallida").Count
$totNc = $tot - $totOk - $totMal
$pct = if ($tot) { [math]::Round(100 * $totOk / $tot, 1) } else { 0 }
$rama = (& "C:\Program Files\Git\cmd\git.exe" -C $Api rev-parse --abbrev-ref HEAD 2>$null)
$commit = (& "C:\Program Files\Git\cmd\git.exe" -C $Api log -1 --format="%h %ad" --date=format:"%d/%m/%Y %H:%M" 2>$null)
$cambiosSinCommit = @(& "C:\Program Files\Git\cmd\git.exe" -C $Api status --porcelain 2>$null).Count
$fallidas = @($Resultados | Where-Object Estado -eq "Fallida")
$noCorridas = @($Resultados | Where-Object { $_.Estado -ne "Aprobada" -and $_.Estado -ne "Fallida" })
$vencenHasta = @($Seleccion | Where-Object { $_.Fin -and $_.Fin.Date -eq $fechaHasta.Date })

# ---- Texto ----
$r = New-Object System.Text.StringBuilder
function L([string]$s = "") { [void]$r.AppendLine($s) }
L ("=" * 100)
L " NOVABOOK - REPORTE DE PRUEBAS UNITARIAS"
if ($NombreOpcion) { L (" " + $NombreOpcion.ToUpper()) }
L " ERP de la libreria Sanborns Saltillo - TecNM Campus Saltillo, ISC"
L " Equipo: Marco Antonio Perez Luevano, Ian Gustavo Herrera Ramirez, Federico Perez Errisuriz, Aldo Alejandro Soto Velez"
L ("=" * 100)
L ""
L "1. DATOS DE LA CORRIDA"
L ("   Fecha y hora .......... {0:dd/MM/yyyy HH:mm:ss} a {1:HH:mm:ss} ({2} s)" -f $inicioTotal, $finTotal, $duracion)
L ("   Alcance ............... $Alcance")
if ($vencenHasta.Count) { L ("   Vencen el {0:dd/MM/yyyy} ..... " -f $fechaHasta + (($vencenHasta | ForEach-Object { "tarea {0}: Pruebas unitarias de {1} (CU-{2:D2})" -f $_.Tarea, $_.Nombre, $_.Num }) -join "; ")) }
L ("   Equipo ................ NovaBook - Windows")
L ("   Herramientas .......... $(& $Rustc --version) - MariaDB $versionMaria (XAMPP)")
L ("   Codigo probado ........ API-Novabook, rama $rama, ultimo commit $commit" + $(if ($cambiosSinCommit) { " (+$cambiosSinCommit archivo(s) con cambios sin commit)" } else { "" }))
L ("   Base de datos ......... Libreria_pruebas (se crea en cada corrida; la base Libreria no se toca)")
L ""
L "2. QUE SE PROBO Y COMO"
L "   Cada caso de uso tiene un archivo en API-Novabook/src/pruebas (cuNN_*.rs). Cada prueba llama a la API real"
L "   dentro del mismo proceso (sin red): inicio de sesion con JWT, permisos por rol, controlador, servicio,"
L "   repositorio y base de datos; despues revisa la respuesta (codigo HTTP y mensaje) y, cuando aplica, el efecto"
L "   en la base (stock por mueble, apartados, bitacora, estados). Cada prueba crea sus propios datos, asi que no"
L "   depende de las demas. Codigos: 200 correcto; 400 dato invalido; 401 sin sesion; 403 rol sin permiso;"
L "   404 no existe; 409 conflicto con el estado actual."
L ""
L "3. RESUMEN"
L ("   Pruebas ejecutadas: $tot   Aprobadas: $totOk   Fallidas: $totMal   No ejecutadas: $totNc   Aprobacion: $pct %")
L ""
L ("   {0,-6} {1,-6} {2,-42} {3,-5} {4,-12} {5,7} {6,9} {7,8} {8,13}" -f "CU", "Tarea", "Caso de uso", "RF", "Fecha", "Pruebas", "Aprobadas", "Fallidas", "No ejecutadas")
L ("   " + ("-" * 114))
foreach ($s in $Resumen) {
    $fecha = if ($s.Caso.Fin) { $s.Caso.Fin.ToString("dd/MM/yyyy") } else { "agregado" }
    L ("   {0,-6} {1,-6} {2,-42} {3,-5} {4,-12} {5,7} {6,9} {7,8} {8,13}" -f ("CU-{0:D2}" -f $s.Caso.Num), $(if ($s.Caso.Tarea) { $s.Caso.Tarea } else { "-" }),
        $s.Caso.Nombre, $s.Caso.RF, $fecha, $s.Total, $s.Ok, $s.Fallas, $s.NoCorridas)
}
L ("   " + ("-" * 114))
L ("   {0,-81} {1,7} {2,9} {3,8} {4,13}" -f "TOTAL", $tot, $totOk, $totMal, $totNc)
L ""
L ("   Comando: cargo test -- $filtros")
L ("   Tiempo de la corrida de pruebas: $segCorrida s" + $(if (-not $termino) { " (SE DETUVO AL PASAR $LimiteMin MIN)" } else { "" }))
L ""
L "4. PRUEBAS QUE NO PASARON"
if ($fallidas.Count -eq 0) { L "   Ninguna: todas las pruebas ejecutadas pasaron." }
foreach ($f in $fallidas) {
    L ("   - {0} (CU-{1:D2}) {2}" -f $f.Id, $f.CU, $f.Titulo)
    L ("       Esperado ...: {0}" -f $f.Esperado)
    L ("       Obtenido ...: {0}" -f $f.Obtenido)
    if ($f.Defecto) { L ("       Causa ......: {0}" -f $f.Defecto) } else { L "       Causa ......: falla nueva; revisar el codigo de la API o de la prueba." }
}
L ""
L "5. PRUEBAS NO EJECUTADAS"
if ($noCorridas.Count -eq 0) { L "   Ninguna: todas las pruebas documentadas se ejecutaron." }
foreach ($f in $noCorridas) { L ("   - {0} (CU-{1:D2}) {2}: {3}" -f $f.Id, $f.CU, $f.Titulo, $f.Obtenido) }
L ""
L "6. DETALLE DE CADA PRUEBA"
foreach ($s in $Resumen) {
    L ""
    L ("   CU-{0:D2} - {1}  ({2}{3}; archivo src/pruebas/{4})" -f $s.Caso.Num, $s.Caso.Nombre, $s.Caso.RF,
        $(if ($s.Caso.Tarea) { "; tarea {0} del cronograma, {1:dd/MM/yyyy}" -f $s.Caso.Tarea, $s.Caso.Fin } else { "; caso agregado" }), $s.Archivo)
    L ("   " + ("-" * 97))
    foreach ($p in ($Resultados | Where-Object CU -eq $s.Caso.Num | Sort-Object Id)) {
        L ("   [{0}] {1} - {2}" -f $p.Estado.ToUpper(), $p.Id, $p.Titulo)
        L ("        Entrada ...: {0}" -f $p.Entrada)
        L ("        Esperado ..: {0}" -f $p.Esperado)
        L ("        Obtenido ..: {0}" -f $p.Obtenido)
    }
}
L ""
L "7. CONCLUSION"
if ($totMal -eq 0 -and $totNc -eq 0) {
    L ("   Las $tot pruebas de los casos de uso del alcance pasaron. Los casos de uso cumplen sus reglas de negocio,")
    L "   validaciones y permisos por rol."
} else {
    L ("   Pasaron $totOk de $tot pruebas ($pct %). " + $(if ($totMal) { "Fallaron $totMal" } else { "" }) + $(if ($totNc) { "; $totNc no se ejecutaron" } else { "" }) + ".")
    $conocidas = @($fallidas | Where-Object Defecto).Count
    if ($conocidas) { L "   $conocidas de las fallas corresponden a defectos ya documentados (D-01/D-02), pendientes para la tarea 102" ; L "   'Correccion de errores post-pruebas' (01 al 03/12/2026)." }
    if ($totMal - $conocidas) { L ("   {0} falla(s) son nuevas y deben revisarse." -f ($totMal - $conocidas)) }
}
L ""
L ("Archivos: Reporte_pruebas_unitarias.txt / .html, resultados.csv y salida_cargo_test.log en $Carpeta")
[IO.File]::WriteAllText((Join-Path $Carpeta "Reporte_pruebas_unitarias.txt"), $r.ToString(), $utf8)

# ---- CSV ----
$Resultados | Sort-Object CU, Id | Select-Object @{n = "CU"; e = { "CU-{0:D2}" -f $_.CU } }, Id, Titulo, Entrada, Esperado, Estado, Obtenido, Defecto, Funcion |
    Export-Csv -Path (Join-Path $Carpeta "resultados.csv") -NoTypeInformation -Encoding UTF8

# ---- HTML ----
function Escape-Html([string]$s) { [Net.WebUtility]::HtmlEncode($s) }
$h = New-Object System.Text.StringBuilder
function A([string]$s) { [void]$h.AppendLine($s) }
A "<!doctype html><html lang='es'><head><meta charset='utf-8'><title>NovaBook - Reporte de pruebas unitarias</title><style>"
A "@page{size:letter landscape;margin:14mm} body{font-family:'Segoe UI',Arial,sans-serif;font-size:10pt;color:#1d1d22;margin:18px}"
A "h1{color:#3B3486;margin:0} h2{color:#2E74B5;font-weight:normal;margin:18px 0 6px;break-after:avoid} .sub{color:#666}"
A "table{border-collapse:collapse;width:100%;margin:6px 0 12px} thead{display:table-header-group} tr{break-inside:avoid}"
A "th{background:#3B3486;color:#fff;text-align:left;padding:4px 6px} td{border:1px solid #bbb;padding:4px 6px;vertical-align:top}"
A ".ok{color:#1E7B34;font-weight:bold} .mal{color:#B00020;font-weight:bold} .nc{color:#A66300;font-weight:bold} .mono{font-family:Consolas,monospace}"
A ".tarjetas{display:flex;gap:10px;margin:10px 0} .t{border:1px solid #ccc;border-radius:6px;padding:8px 14px} .t b{font-size:18pt;display:block}</style></head><body>"
A "<h1>NovaBook &mdash; Reporte de pruebas unitarias</h1>"
if ($NombreOpcion) { A ("<p style='font-size:13pt;color:#3B3486;margin:4px 0'><b>{0}</b></p>" -f (Escape-Html $NombreOpcion)) }
A ("<p class='sub'>{0:dd/MM/yyyy HH:mm} &middot; {1} &middot; {2} s</p>" -f $inicioTotal, (Escape-Html $Alcance), $duracion)
A ("<div class='tarjetas'><div class='t'>Ejecutadas<b>$tot</b></div><div class='t'>Aprobadas<b class='ok'>$totOk</b></div><div class='t'>Fallidas<b class='mal'>$totMal</b></div><div class='t'>No ejecutadas<b class='nc'>$totNc</b></div><div class='t'>Aprobaci&oacute;n<b>$pct %</b></div></div>")
A ("<p>Equipo: {0} &middot; {1} &middot; MariaDB {2} &middot; API-Novabook rama <span class='mono'>{3}</span>, commit {4}</p>" -f "NovaBook", (Escape-Html (& $Rustc --version)), (Escape-Html $versionMaria), (Escape-Html $rama), (Escape-Html $commit))
A "<h2>Resumen por caso de uso</h2><table><thead><tr><th>CU</th><th>Tarea</th><th>Caso de uso</th><th>RF</th><th>Fecha</th><th>Pruebas</th><th>Aprobadas</th><th>Fallidas</th><th>No ejecutadas</th></tr></thead><tbody>"
foreach ($s in $Resumen) {
    A ("<tr><td>CU-{0:D2}</td><td>{1}</td><td>{2}</td><td>{3}</td><td>{4}</td><td>{5}</td><td class='ok'>{6}</td><td class='{7}'>{8}</td><td>{9}</td></tr>" -f $s.Caso.Num,
        $(if ($s.Caso.Tarea) { $s.Caso.Tarea } else { "-" }), (Escape-Html $s.Caso.Nombre), $s.Caso.RF, $(if ($s.Caso.Fin) { $s.Caso.Fin.ToString("dd/MM/yyyy") } else { "agregado" }),
        $s.Total, $s.Ok, $(if ($s.Fallas) { "mal" } else { "" }), $s.Fallas, $s.NoCorridas)
}
A ("<tr><td colspan='5'><b>Total</b></td><td><b>$tot</b></td><td class='ok'>$totOk</td><td class='mal'>$totMal</td><td>$totNc</td></tr></tbody></table>")
A "<h2>Pruebas que no pasaron</h2>"
if ($fallidas.Count -eq 0) { A "<p>Ninguna: todas las pruebas ejecutadas pasaron.</p>" } else {
    A "<table><thead><tr><th>ID</th><th>Prueba</th><th>Esperado</th><th>Obtenido</th><th>Causa</th></tr></thead><tbody>"
    foreach ($f in $fallidas) { A ("<tr><td class='mono'>{0}</td><td>{1}</td><td>{2}</td><td>{3}</td><td>{4}</td></tr>" -f $f.Id, (Escape-Html $f.Titulo), (Escape-Html $f.Esperado), (Escape-Html $f.Obtenido), (Escape-Html $(if ($f.Defecto) { $f.Defecto } else { "Falla nueva: revisar." }))) }
    A "</tbody></table>"
}
A "<h2>Detalle de cada prueba</h2>"
foreach ($s in $Resumen) {
    A ("<h3>CU-{0:D2} &middot; {1} <span class='sub'>({2}{3})</span></h3>" -f $s.Caso.Num, (Escape-Html $s.Caso.Nombre), $s.Caso.RF, $(if ($s.Caso.Tarea) { "; tarea {0}, {1:dd/MM/yyyy}" -f $s.Caso.Tarea, $s.Caso.Fin } else { "; agregado" }))
    A "<table><thead><tr><th style='width:7%'>ID</th><th style='width:16%'>Prueba</th><th style='width:24%'>Entrada</th><th style='width:24%'>Resultado esperado</th><th style='width:21%'>Resultado obtenido</th><th>Estado</th></tr></thead><tbody>"
    foreach ($p in ($Resultados | Where-Object CU -eq $s.Caso.Num | Sort-Object Id)) {
        $cls = switch ($p.Estado) { "Aprobada" { "ok" } "Fallida" { "mal" } default { "nc" } }
        A ("<tr><td class='mono'>{0}</td><td>{1}</td><td>{2}</td><td>{3}</td><td>{4}</td><td class='{5}'>{6}</td></tr>" -f $p.Id, (Escape-Html $p.Titulo), (Escape-Html $p.Entrada), (Escape-Html $p.Esperado), (Escape-Html $p.Obtenido), $cls, $p.Estado)
    }
    A "</tbody></table>"
}
A "</body></html>"
[IO.File]::WriteAllText((Join-Path $Carpeta "Reporte_pruebas_unitarias.html"), $h.ToString(), $utf8)

# -----------------------------------------------------------------------------
# 5. Resumen final en pantalla
# -----------------------------------------------------------------------------
Write-Titulo "4. Resultado"
Write-Host (" Ejecutadas: $tot   Aprobadas: $totOk   Fallidas: $totMal   No ejecutadas: $totNc   ($pct %)   Duracion: $duracion s")
foreach ($f in $fallidas) {
    Write-Host (" x {0} {1}" -f $f.Id, $f.Titulo) -ForegroundColor Red
    if ($f.Defecto) { Write-Host ("     Defecto conocido " + $f.Defecto.Split(":")[0]) -ForegroundColor DarkYellow }
}
Write-Host ""
Write-Host " Reporte: $(Join-Path $Carpeta 'Reporte_pruebas_unitarias.txt')" -ForegroundColor Cyan
Write-Host "          $(Join-Path $Carpeta 'Reporte_pruebas_unitarias.html')" -ForegroundColor Cyan
if ($totMal -gt 0 -or $totNc -gt 0) { exit 1 } else { exit 0 }
