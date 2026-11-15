-- ============================================================================
-- MIGRACIÓN: columna `observaciones` en `devolucion`
-- ----------------------------------------------------------------------------
-- Antes, la nota del Jefe al aprobar/rechazar solo quedaba dentro del texto de
-- la bitácora y la API la recuperaba con un patrón de texto. Ahora se guarda en
-- su propia columna.
--
-- Aplica solo a bases creadas con una versión anterior de
-- libreria_unificada-definitivo.sql (las nuevas ya traen la columna).
-- Es idempotente: se puede correr más de una vez sin efecto adicional.
--
--   mariadb -u <usuario> -p Libreria < migracion_observaciones_devolucion.sql
-- ============================================================================

USE `Libreria`;

ALTER TABLE `devolucion`
  ADD COLUMN IF NOT EXISTS `observaciones` VARCHAR(200) DEFAULT NULL
  AFTER `fecha_aprobacion`;

-- Copia las observaciones ya registradas en la bitácora. Acepta los dos
-- textos que ha usado la API:
--   "Devolución #N rechazada. Observaciones: ..."              (actual)
--   "Devolución #N actualizada a estado 'X'. Observaciones: ..." (anterior)
-- Solo llena las filas vacías y toma el evento más reciente de cada devolución.
UPDATE `devolucion` d
SET d.`observaciones` = LEFT((
      SELECT SUBSTRING_INDEX(b.`detalle`, 'Observaciones: ', -1)
        FROM `bitacora` b
       WHERE (b.`accion` LIKE 'RECHAZAR DEVOLUCI%' OR b.`accion` LIKE 'APROBAR DEVOLUCI%')
         AND b.`detalle` LIKE CONCAT('%#', d.`id_devolucion`, ' %Observaciones: %')
       ORDER BY b.`fecha_hora` DESC, b.`id_bitacora` DESC
       LIMIT 1
    ), 200)
WHERE d.`observaciones` IS NULL
  AND d.`estado` IN ('APROBADO', 'RECHAZADO');

-- Verificación rápida
SELECT `estado`, COUNT(*) AS total, SUM(`observaciones` IS NOT NULL) AS con_observaciones
  FROM `devolucion`
 GROUP BY `estado`;
