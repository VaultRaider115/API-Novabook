@echo off
rem NovaBook - Pruebas unitarias (doble clic para elegir una opcion).
rem Tambien acepta parametros directos, por ejemplo:
rem   Ejecutar_pruebas_unitarias.bat -Opcion 2
rem   Ejecutar_pruebas_unitarias.bat -Solo 13,14
rem Windows bloquea los .ps1 en este equipo; -ExecutionPolicy Bypass lo permite
rem solo para este script, sin cambiar la configuracion del sistema.
if not "%~1"=="" (
    powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0Pruebas_unitarias_NovaBook.ps1" %*
    goto fin
)
:menu
echo.
echo  ============================================================
echo   NovaBook - Pruebas unitarias
echo  ============================================================
echo   1. Revision segun cronograma antes de cambios  (CU-01 a CU-14)
echo   2. Hito segun cronograma antes de cambios      (CU-01 a CU-19)
echo   3. Revision segun cronograma actualizado       (CU-01 a CU-25)
echo   4. Hito segun cronograma actualizado           (CU-01 a CU-32)
echo   0. Salir
echo.
set "op="
set /p "op=  Elige una opcion: "
if "%op%"=="0" goto :eof
if "%op%"=="1" goto correr
if "%op%"=="2" goto correr
if "%op%"=="3" goto correr
if "%op%"=="4" goto correr
echo   Opcion no valida.
goto menu
:correr
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0Pruebas_unitarias_NovaBook.ps1" -Opcion %op%
:fin
echo.
pause
