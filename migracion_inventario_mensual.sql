-- ============================================================================
-- MIGRACIÓN: inventario mensual y solicitudes de editorial
-- (diagrama de procesos, paso 7)
-- ----------------------------------------------------------------------------
-- - inventario / detalle_inventario: conteo físico del piso de venta. El
--   Vendedor escanea y cierra; el Jefe aprueba (ajusta piso con la diferencia
--   medida al cierre) o rechaza. Al cerrar se marca qué va a devolución.
-- - solicitud_editorial / detalle_solicitud_editorial: EAN que una editorial
--   pidió de vuelta (por correo); se marcan en el inventario mientras estén
--   ABIERTAS.
-- Idempotente. Las bases nuevas ya lo traen en libreria_unificada-definitivo.sql.
-- ============================================================================

USE `Libreria`;

CREATE TABLE IF NOT EXISTS `solicitud_editorial` (
  `id_solicitud` BIGINT       NOT NULL AUTO_INCREMENT,
  `id_proveedor` INT          NOT NULL,
  `id_usuarios`  BIGINT       NOT NULL,                       -- quien la registró
  `fecha`        DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
  `referencia`   VARCHAR(150) NOT NULL,                       -- p. ej. asunto/fecha del correo
  `estado`       VARCHAR(10)  NOT NULL DEFAULT 'ABIERTA',     -- ABIERTA | ATENDIDA
  PRIMARY KEY (`id_solicitud`),
  KEY `FK_SE_PROVEEDOR` (`id_proveedor`),
  KEY `FK_SE_USUARIO` (`id_usuarios`),
  CONSTRAINT `FK_SE_PROVEEDOR` FOREIGN KEY (`id_proveedor`) REFERENCES `proveedor` (`id_proveedor`),
  CONSTRAINT `FK_SE_USUARIO`   FOREIGN KEY (`id_usuarios`)  REFERENCES `usuarios` (`id_usuarios`),
  CONSTRAINT `CK_SE_ESTADO`    CHECK (`estado` IN ('ABIERTA', 'ATENDIDA'))
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_general_ci;

CREATE TABLE IF NOT EXISTS `detalle_solicitud_editorial` (
  `id_solicitud` BIGINT NOT NULL,
  `codigo_ean`   BIGINT NOT NULL,
  PRIMARY KEY (`id_solicitud`, `codigo_ean`),
  KEY `FK_DSE_PRODUCTO` (`codigo_ean`),
  CONSTRAINT `FK_DSE_SOLICITUD` FOREIGN KEY (`id_solicitud`) REFERENCES `solicitud_editorial` (`id_solicitud`) ON DELETE CASCADE,
  CONSTRAINT `FK_DSE_PRODUCTO`  FOREIGN KEY (`codigo_ean`)   REFERENCES `productos` (`codigo_ean`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_general_ci;

CREATE TABLE IF NOT EXISTS `inventario` (
  `id_inventario`      BIGINT       NOT NULL AUTO_INCREMENT,
  `id_usuarios`        BIGINT       NOT NULL,                 -- quien contó (Vendedor)
  `fecha_inicio`       DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
  `fecha_cierre`       DATETIME     DEFAULT NULL,
  `estado`             VARCHAR(10)  NOT NULL DEFAULT 'ABIERTO', -- ABIERTO | CERRADO | APROBADO | RECHAZADO
  `dias_anejamiento`   INT          NOT NULL DEFAULT 90,       -- regla usada al cerrar
  `id_usuario_revisa`  BIGINT       DEFAULT NULL,              -- Jefe que aprobó/rechazó
  `fecha_revision`     DATETIME     DEFAULT NULL,
  `observaciones`      VARCHAR(200) DEFAULT NULL,
  PRIMARY KEY (`id_inventario`),
  KEY `FK_INV_USUARIO` (`id_usuarios`),
  KEY `FK_INV_REVISA` (`id_usuario_revisa`),
  CONSTRAINT `FK_INV_USUARIO` FOREIGN KEY (`id_usuarios`)       REFERENCES `usuarios` (`id_usuarios`),
  CONSTRAINT `FK_INV_REVISA`  FOREIGN KEY (`id_usuario_revisa`) REFERENCES `usuarios` (`id_usuarios`),
  CONSTRAINT `CK_INV_ESTADO`  CHECK (`estado` IN ('ABIERTO', 'CERRADO', 'APROBADO', 'RECHAZADO'))
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_general_ci;

CREATE TABLE IF NOT EXISTS `detalle_inventario` (
  `id_inventario`     BIGINT      NOT NULL,
  `codigo_ean`        BIGINT      NOT NULL,
  `cantidad_contada`  INT         NOT NULL DEFAULT 0,
  `stock_sistema`     INT         DEFAULT NULL,   -- piso al cerrar
  `stock_apartado`    INT         NOT NULL DEFAULT 0,
  `motivo_devolucion` VARCHAR(20) DEFAULT NULL,   -- ANEJAMIENTO | SOLICITUD_EDITORIAL
  PRIMARY KEY (`id_inventario`, `codigo_ean`),
  KEY `FK_DI_PRODUCTO` (`codigo_ean`),
  CONSTRAINT `FK_DI_INVENTARIO` FOREIGN KEY (`id_inventario`) REFERENCES `inventario` (`id_inventario`) ON DELETE CASCADE,
  CONSTRAINT `FK_DI_PRODUCTO`   FOREIGN KEY (`codigo_ean`)    REFERENCES `productos` (`codigo_ean`),
  CONSTRAINT `CK_DI_CONTADA`    CHECK (`cantidad_contada` >= 0)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_general_ci;
