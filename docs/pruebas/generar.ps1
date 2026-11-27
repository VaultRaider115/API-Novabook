# Genera "Pruebas unitarias NovaBook" (.docx y .pdf) desde una corrida real de cargo test.
# Uso (PowerShell, desde cualquier carpeta):
#   .\docs\pruebas\generar.ps1                      # corre cargo test y genera el documento
#   .\docs\pruebas\generar.ps1 -SinCorrer           # reutiliza resultados_cargo_test.txt
#   .\docs\pruebas\generar.ps1 -Salida C:\otra\carpeta
# Requiere: MySQL de XAMPP encendido (para cargo test), Python 3 con python-docx
# (pip install python-docx) y Microsoft Edge (para el PDF). No automatiza Word.
param(
    [switch]$SinCorrer,
    [string]$Salida = (Join-Path $PSScriptRoot "..\..\..\01_Documentacion\NovaBook-actualizado\01_Documentos")
)
$ErrorActionPreference = "Stop"
$api = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$resultados = Join-Path $PSScriptRoot "resultados_cargo_test.txt"
$enc = New-Object Text.UTF8Encoding($false)

if (-not $SinCorrer) {
    $cargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"
    $rustc = Join-Path $env:USERPROFILE ".cargo\bin\rustc.exe"
    $inicio = Get-Date
    Push-Location $api
    # Por cmd: cargo escribe el avance en stderr y PowerShell 5.1 lo tomaría como error.
    try { $out = cmd /c "`"$cargo`" test 2>&1" } finally { Pop-Location }
    $fin = Get-Date
    $maria = & "C:\xampp\mysql\bin\mysql.exe" -u root -N -e "SELECT VERSION();"
    $cab = "# Corrida de cargo test`n# Inicio: $($inicio.ToString('yyyy-MM-dd HH:mm:ss'))  Fin: $($fin.ToString('yyyy-MM-dd HH:mm:ss'))`n" +
           "# Equipo: NovaBook · $(& $rustc --version) · MariaDB $maria`n"
    [IO.File]::WriteAllText($resultados, $cab + ($out -join "`n"), $enc)
    ($out | Select-String "^test result").Line
}

$python = Get-Command python -ErrorAction SilentlyContinue | Where-Object { $_.Source -notlike "*WindowsApps*" } | Select-Object -First 1
$py = if ($python) { $python.Source } else { Join-Path $env:LOCALAPPDATA "Programs\Python\Python312\python.exe" }

New-Item -ItemType Directory -Force $Salida | Out-Null
$dir = (Resolve-Path $Salida).Path
$docx = Join-Path $dir "Pruebas unitarias NovaBook.docx"
$pdf = Join-Path $dir "Pruebas unitarias NovaBook.pdf"
$html = Join-Path ([IO.Path]::GetTempPath()) "novabook_pruebas_unitarias.html"

# Sin -I: el modo aislado ignora las bibliotecas del usuario (ahí queda python-docx).
& $py (Join-Path $PSScriptRoot "generar_documento.py") $api $docx $html
if ($LASTEXITCODE -ne 0) { throw "generar_documento.py falló" }

# PDF con Edge sin ventana (como 02_Entregas/entregables/fuente/render.sh).
$edge = "${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe"
if (-not (Test-Path $edge)) { $edge = "$env:ProgramFiles\Microsoft\Edge\Application\msedge.exe" }
$url = "file:///" + $html.Replace('\', '/')
$p = Start-Process -FilePath $edge -ArgumentList @(
    "--headless=new", "--disable-gpu", "--no-pdf-header-footer", "--allow-file-access-from-files",
    "--virtual-time-budget=10000", "--print-to-pdf=`"$pdf`"", $url
) -PassThru -Wait -WindowStyle Hidden
if (-not (Test-Path $pdf)) { throw "Edge no generó el PDF (código $($p.ExitCode))" }
"PDF:  $pdf"
