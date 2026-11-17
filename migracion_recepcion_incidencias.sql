-- ============================================================================
-- MIGRACIÓN: incidencias de compra y recepción de requisiciones
-- (diagrama de procesos, pasos 1, 2 y 3)
-- ----------------------------------------------------------------------------
-- - incidencia_compra / detalle_incidencia_compra: la factura dice N piezas y
--   llegaron menos. Se abre al registrar la compra; Bodega avisa al proveedor
--   por correo y la cierra al recibir los faltantes.
-- - requisicion / detalle_requisicion: cada requisición con folio REQ-AAAA-n.
--   La de salida (bodega → piso) queda PENDIENTE hasta que el Vendedor la
--   recibe: confirma lo que llegó (o la corrige, CORREGIDA) y elige el mueble
--   de piso de cada libro. La de entrada queda REGISTRADA.
-- Idempotente. Las bases nuevas ya lo traen en libreria_unificada-definitivo.sql.
-- ============================================================================

USE `Libreria`;

CREATE TABLE IF NOT EXISTS `incidencia_compra` (
  `id_incidencia`        BIGINT      NOT NULL AUTO_INCREMENT,
  `id_compras_proveedor` BIGINT      NOT NULL,
  `id_usuarios`          BIGINT      NOT NULL,                  -- quien recibió el paquete
  `fecha`                DATETIME    NOT NULL DEFAULT CURRENT_TIMESTAMP,
  `estado`               VARCHAR(10) NOT NULL DEFAULT 'ABIERTA', -- ABIERTA | CERRADA
  `fecha_cierre`         DATETIME    DEFAULT NULL,
  PRIMARY KEY (`id_incidencia`),
  UNIQUE KEY `UQ_IC_COMPRA` (`id_compras_proveedor`),
  KEY `FK_IC_USUARIO` (`id_usuarios`),
  CONSTRAINT `FK_IC_COMPRA`  FOREIGN KEY (`id_compras_proveedor`) REFERENCES `compras_proveedor` (`id_compras_proveedor`) ON DELETE CASCADE,
  CONSTRAINT `FK_IC_USUARIO` FOREIGN KEY (`id_usuarios`)          REFERENCES `usuarios` (`id_usuarios`),
  CONSTRAINT `CK_IC_ESTADO`  CHECK (`estado` IN ('ABIERTA', 'CERRADA'))
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_general_ci;

CREATE TABLE IF NOT EXISTS `detalle_incidencia_compra` (
  `id_incidencia`      BIGINT        NOT NULL,
  `codigo_ean`         BIGINT        NOT NULL,
  `cantidad_facturada` INT           NOT NULL,
  `cantidad_recibida`  INT           NOT NULL,             -- lo que llegó con la factura
  `cantidad_repuesta`  INT           NOT NULL DEFAULT 0,   -- lo que llegó después
  `costo_unitario`     DECIMAL(10,2) NOT NULL DEFAULT 0.00,
  PRIMARY KEY (`id_incidencia`, `codigo_ean`),
  KEY `FK_DIC_PRODUCTO` (`codigo_ean`),
  CONSTRAINT `FK_DIC_INCIDENCIA` FOREIGN KEY (`id_incidencia`) REFERENCES `incidencia_compra` (`id_incidencia`) ON DELETE CASCADE,
  CONSTRAINT `FK_DIC_PRODUCTO`   FOREIGN KEY (`codigo_ean`)    REFERENCES `productos` (`codigo_ean`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_general_ci;

CREATE TABLE IF NOT EXISTS `requisicion` (
  `folio`             VARCHAR(30)  NOT NULL,                  -- REQ-AAAA-n
  `tipo`              VARCHAR(10)  NOT NULL,                  -- SALIDA | ENTRADA
  `id_proveedor`      INT          DEFAULT NULL,              -- editorial del documento
  `id_usuarios`       BIGINT       NOT NULL,                  -- quien la registró
  `fecha`             DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
  `estado`            VARCHAR(10)  NOT NULL DEFAULT 'PENDIENTE', -- PENDIENTE | CONFIRMADA | CORREGIDA | REGISTRADA
  `id_usuario_recibe` BIGINT       DEFAULT NULL,              -- Vendedor que la recibió
  `fecha_recepcion`   DATETIME     DEFAULT NULL,
  `observaciones`     VARCHAR(200) DEFAULT NULL,
  PRIMARY KEY (`folio`),
  KEY `FK_REQ_PROVEEDOR` (`id_proveedor`),
  KEY `FK_REQ_USUARIO` (`id_usuarios`),
  KEY `FK_REQ_RECIBE` (`id_usuario_recibe`),
  CONSTRAINT `FK_REQ_PROVEEDOR` FOREIGN KEY (`id_proveedor`)      REFERENCES `proveedor` (`id_proveedor`),
  CONSTRAINT `FK_REQ_USUARIO`   FOREIGN KEY (`id_usuarios`)       REFERENCES `usuarios` (`id_usuarios`),
  CONSTRAINT `FK_REQ_RECIBE`    FOREIGN KEY (`id_usuario_recibe`) REFERENCES `usuarios` (`id_usuarios`),
  CONSTRAINT `CK_REQ_TIPO`      CHECK (`tipo` IN ('SALIDA', 'ENTRADA')),
  CONSTRAINT `CK_REQ_ESTADO`    CHECK (`estado` IN ('PENDIENTE', 'CONFIRMADA', 'CORREGIDA', 'REGISTRADA'))
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_general_ci;

CREATE TABLE IF NOT EXISTS `detalle_requisicion` (
  `folio`             VARCHAR(30) NOT NULL,
  `codigo_ean`        BIGINT      NOT NULL,
  `cantidad`          INT         NOT NULL,      -- lo que dice el documento original
  `cantidad_recibida` INT         DEFAULT NULL,  -- lo que confirmó el Vendedor
  `id_mueble`         INT         NOT NULL,      -- mueble destino (en piso, para la salida)
  PRIMARY KEY (`folio`, `codigo_ean`),
  KEY `FK_DR_PRODUCTO` (`codigo_ean`),
  KEY `FK_DR_MUEBLE` (`id_mueble`),
  CONSTRAINT `FK_DR_REQUISICION` FOREIGN KEY (`folio`)      REFERENCES `requisicion` (`folio`) ON DELETE CASCADE,
  CONSTRAINT `FK_DR_PRODUCTO`    FOREIGN KEY (`codigo_ean`) REFERENCES `productos` (`codigo_ean`),
  CONSTRAINT `FK_DR_MUEBLE`      FOREIGN KEY (`id_mueble`)  REFERENCES `mueble` (`id_mueble`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_general_ci;
