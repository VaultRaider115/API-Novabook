-- ============================================================================
-- MIGRACIÓN: devolución de cliente con ticket (diagrama de procesos, paso 5)
-- ----------------------------------------------------------------------------
-- - apartados: libros con defecto que regresó un cliente; no se venden ni se
--   trasladan. Salen solo al devolverlos a la editorial (paso 8).
-- - devolucion_cliente / detalle_devolucion_cliente: devolución de un ticket
--   (máx. 15 días), con reembolso o cambio por productos de igual o mayor
--   precio (la venta del cambio queda ligada en id_venta_cambio).
-- Idempotente. Las bases nuevas ya lo traen en libreria_unificada-definitivo.sql.
--   mariadb -u <usuario> -p Libreria < migracion_devolucion_cliente.sql
-- ============================================================================

USE `Libreria`;

CREATE TABLE IF NOT EXISTS `apartados` (
  `codigo_ean` BIGINT NOT NULL,
  `cantidad`   INT    NOT NULL DEFAULT 0,
  PRIMARY KEY (`codigo_ean`),
  CONSTRAINT `FK_APARTADOS_PRODUCTO` FOREIGN KEY (`codigo_ean`) REFERENCES `productos` (`codigo_ean`),
  CONSTRAINT `CK_APARTADOS_CANTIDAD` CHECK (`cantidad` >= 0)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_general_ci;

CREATE TABLE IF NOT EXISTS `devolucion_cliente` (
  `id_devolucion_cliente` BIGINT        NOT NULL AUTO_INCREMENT,
  `id_ventas`             BIGINT        NOT NULL,              -- ticket original
  `id_usuarios`           BIGINT        NOT NULL,              -- quien la registró
  `fecha_hora`            DATETIME      NOT NULL DEFAULT CURRENT_TIMESTAMP,
  `resolucion`            VARCHAR(10)   NOT NULL,              -- REEMBOLSO | CAMBIO
  `total_devuelto`        DECIMAL(10,2) NOT NULL,              -- a precio pagado en el ticket
  `id_venta_cambio`       BIGINT        DEFAULT NULL,          -- venta nueva si fue CAMBIO
  `diferencia_cobrada`    DECIMAL(10,2) NOT NULL DEFAULT 0.00,
  `observaciones`         VARCHAR(200)  DEFAULT NULL,
  PRIMARY KEY (`id_devolucion_cliente`),
  KEY `FK_DC_VENTA` (`id_ventas`),
  KEY `FK_DC_USUARIO` (`id_usuarios`),
  KEY `FK_DC_VENTA_CAMBIO` (`id_venta_cambio`),
  CONSTRAINT `FK_DC_VENTA`        FOREIGN KEY (`id_ventas`)       REFERENCES `ventas` (`id_ventas`),
  CONSTRAINT `FK_DC_USUARIO`      FOREIGN KEY (`id_usuarios`)     REFERENCES `usuarios` (`id_usuarios`),
  CONSTRAINT `FK_DC_VENTA_CAMBIO` FOREIGN KEY (`id_venta_cambio`) REFERENCES `ventas` (`id_ventas`),
  CONSTRAINT `CK_DC_RESOLUCION`   CHECK (`resolucion` IN ('REEMBOLSO', 'CAMBIO'))
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_general_ci;

CREATE TABLE IF NOT EXISTS `detalle_devolucion_cliente` (
  `id_detalle`            BIGINT        NOT NULL AUTO_INCREMENT,
  `id_devolucion_cliente` BIGINT        NOT NULL,
  `codigo_ean`            BIGINT        NOT NULL,
  `cantidad`              INT           NOT NULL,
  `precio_unitario`       DECIMAL(10,2) NOT NULL,               -- precio pagado en el ticket
  `con_defecto`           BOOLEAN       NOT NULL DEFAULT 0,     -- 1 → apartados; 0 → piso
  PRIMARY KEY (`id_detalle`),
  KEY `FK_DDC_DEVOLUCION` (`id_devolucion_cliente`),
  KEY `FK_DDC_PRODUCTO` (`codigo_ean`),
  CONSTRAINT `FK_DDC_DEVOLUCION` FOREIGN KEY (`id_devolucion_cliente`) REFERENCES `devolucion_cliente` (`id_devolucion_cliente`) ON DELETE CASCADE,
  CONSTRAINT `FK_DDC_PRODUCTO`   FOREIGN KEY (`codigo_ean`) REFERENCES `productos` (`codigo_ean`),
  CONSTRAINT `CK_DDC_CANTIDAD`   CHECK (`cantidad` > 0)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_general_ci;
