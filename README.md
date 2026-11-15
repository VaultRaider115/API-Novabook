# NovaBook — API REST (Rust)

Backend del ERP de la librería Sanborns Saltillo (TecNM Campus Saltillo, ISC).
Expone la API REST que consume el cliente de escritorio Qt de
`../Frontend-NovaBook`.

- **Stack:** Rust 2024, Axum 0.8, SQLx 0.9 (MySQL/MariaDB), JWT y bcrypt.
- **Capas:** controller → service → repo → model.

| Documento | Contenido |
|---|---|
| [`AI_CONTEXT.md`](AI_CONTEXT.md) | arquitectura, rutas por rol, reglas de negocio y migraciones |
| [`CHANGELOG.md`](CHANGELOG.md) | historial de cambios |

## 1. Requisitos

- Rust estable con edición 2024 (`rustup`, 1.85 o posterior)
- MySQL 8 o MariaDB 10.11 (en local, la MariaDB de XAMPP sirve)

## 2. Base de datos

Importa el esquema unificado (BD `Libreria`), que incluye los datos de
prueba. **Reimportarlo borra la base.**

```bash
mysql -u root -e "source libreria_unificada-definitivo.sql"
```

Con XAMPP en Windows (PowerShell):

```powershell
& "C:\xampp\mysql\bin\mysql.exe" -u root -e "source C:/Proyectos/NovaBook-v2/docs/libreria_unificada-definitivo.sql"
```

`libreria_unificada-definitivo.sql` es una copia idéntica de
`../docs/libreria_unificada-definitivo.sql`.

Si la base se creó antes de alguno de estos cambios, aplica sus migraciones
(son idempotentes):

| Migración | Agrega |
|---|---|
| `migracion_observaciones_devolucion.sql` | columna `devolucion.observaciones` |
| `migracion_devolucion_cliente.sql` | devolución de cliente y tabla `apartados` |
| `migracion_inventario_mensual.sql` | inventario mensual y solicitudes de editorial |
| `migracion_recepcion_incidencias.sql` | incidencias de compra incompleta y recepción de requisiciones |

## 3. Configuración

Crea un archivo `.env` en esta carpeta. No se sube al repositorio: está en
`.gitignore`.

```dotenv
DATABASE_URL=mysql://root@127.0.0.1:3306/Libreria
JWT_SECRET=pon-aqui-una-cadena-larga-y-aleatoria
SERVER_HOST=127.0.0.1
SERVER_PORT=8080
# JWT_EXPIRATION_HOURS=24
```

| Variable | Obligatoria | Default |
|---|---|---|
| `DATABASE_URL` | sí | — |
| `JWT_SECRET` | sí | — |
| `JWT_EXPIRATION_HOURS` | no | 24 |
| `SERVER_HOST` | no | `0.0.0.0` |
| `SERVER_PORT` | no | 8080 |

## 4. Ejecutar

```bash
cargo run
```

Al arrancar, el log muestra la URL de la base con la contraseña oculta y
`Servidor REST NovaBook ejecutándose en http://127.0.0.1:8080`.

> En Windows, detén la API antes de `cargo build`: el `.exe` en uso queda
> bloqueado. Mientras corre, usa `cargo check`.

Prueba rápida:

```bash
curl -s -H "Content-Type: application/json" \
  -d '{"id_usuario":628777129,"contrasena":"8888"}' \
  http://127.0.0.1:8080/api/auth/login
```

### Usuarios de prueba

| Usuario | Contraseña | Rol |
|---|---|---|
| 1001 | admin123 | Gerente |
| 350976899 | 2501 | Jefe de Departamento |
| 628777130 | 7777 | Personal de Bodega |
| 628777129 | 8888 | Vendedor |

## 5. Pruebas

```bash
cargo test                    # pruebas unitarias
cargo clippy --all-targets    # debe quedar sin avisos
```

**De punta a punta** (`test_e2e.sh`, 11 secciones): primero importa una base
limpia y deja la API corriendo. Después:

```bash
bash test_e2e.sh
```

En Windows con Git Bash usa el curl de System32 (el de Git Bash rompe los
acentos) y quita los retornos de carro del script:

```bash
tr -d '\r' < test_e2e.sh > /tmp/e2e.sh && PATH=/c/WINDOWS/system32:$PATH bash /tmp/e2e.sh
```

`BASE_URL` cambia la dirección de la API (default `http://127.0.0.1:8080/api`).

## 6. Despliegue

- **Docker:** `Dockerfile` multi-etapa (Ubuntu 24.04). Compila en release y
  publica el binario `api_migrado` en el puerto 8080.
- **CI:** `.github/workflows/deploy.yaml` construye la imagen al hacer push a
  `main` y la publica en GHCR (`ghcr.io/<owner>/api-novabook:latest`).
- **K3s:** `k3s/database.yaml` levanta MariaDB 10.11 con volumen persistente
  y `k3s/backend.yaml` despliega la API. Ambos leen las credenciales del
  secreto `novabook-secrets`.

## 7. Estructura

```text
API-Novabook/
├── src/
│   ├── main.rs, config.rs, db.rs, error.rs
│   ├── middleware/auth.rs     JWT y guardas por rol
│   ├── usuarios/              login, perfil y CRUD de usuarios
│   ├── productos/             catálogos, stock, movimientos, ventas, compras,
│   │                          devoluciones, inventario mensual y bitácora
│   └── services/pdf.rs        acta de devolución en PDF
├── libreria_unificada-definitivo.sql, seeds.sql, migracion_*.sql
├── test_e2e.sh
├── Dockerfile, k3s/
└── AI_CONTEXT.md, CHANGELOG.md
```
