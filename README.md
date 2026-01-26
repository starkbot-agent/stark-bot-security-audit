# StarkBot

A cloud-deployable agentic assistant built with Rust and Actix. StarkBot acts as an intelligent automation hub that can interface with messaging platforms (WhatsApp, Slack), email services (Gmail), and more. Deploy it to the cloud and let it handle conversations, automate workflows, and integrate with your favorite services.

**Key Features:**
- Multi-platform messaging integration (WhatsApp, Slack, Gmail, and more)
- Agentic AI capabilities for intelligent conversation handling
- Secure session management with SQLite storage
- Easy cloud deployment (DigitalOcean, AWS, etc.)
- Hot-reload development environment




## Starkbot is in early BETA 

 <img width="1453" height="1018" alt="image" src="https://github.com/user-attachments/assets/2e51cf69-9c77-4610-a392-41ebd21993a5" />



## Local Development

### Prerequisites

- Rust 1.88+ (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- SQLite3 (usually pre-installed on Linux)

### Generate a Secret Key

```bash
# Using openssl (recommended)
openssl rand -base64 32

# Using /dev/urandom
head -c 32 /dev/urandom | base64

# Using uuid
cat /proc/sys/kernel/random/uuid
```

### Environment Setup

Both local development and Docker configurations read from a `.env` file. Set this up once and all commands will use it automatically.

```bash
# Copy environment template
cp .env.template .env

# Generate and set your SECRET_KEY
SECRET_KEY=$(openssl rand -base64 32)
sed -i "s/your-secret-key-here/$SECRET_KEY/" .env

# Or manually edit .env
nano .env
```

Your `.env` file should contain:
```
SECRET_KEY=<your-generated-key>
PORT=8080
DATABASE_URL=./.db/stark.db
RUST_LOG=info
```

### Configure AI (After First Login)

API keys are managed through the web UI, not environment variables:

1. Start the server and login
2. Go to **API Keys** in the sidebar
3. Add your Anthropic API key (get one from [console.anthropic.com](https://console.anthropic.com/))
4. Your key is stored securely in the local SQLite database

### Run Locally

```bash
# Run the server (reads from .env automatically via dotenv)
cargo run -p stark-backend
```

The server starts at `http://localhost:8080`

### Test Endpoints

```bash
# Health check
curl http://localhost:8080/health

# Login
curl -X POST http://localhost:8080/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"secret_key":"your-secret-key"}'
```

## Local Docker Testing

The production Docker setup reads configuration from your `.env` file automatically (no need to pass `-e` flags).

### Run with Docker Compose (Recommended)

```bash
# Start the container (reads from .env automatically)
docker compose up --build

# Or run in background
docker compose up --build -d

# View logs
docker compose logs -f

# Stop
docker compose down
```

This includes persistent database storage in the `./data` directory.

### Run with Docker Manually

If you prefer manual Docker commands:

```bash
# Build the image
docker build -t starkbot .

# Run with env file
docker run -p 8080:8080 --env-file .env -v $(pwd)/data:/app/.db starkbot

# Or run in detached mode
docker run -d -p 8080:8080 --env-file .env -v $(pwd)/data:/app/.db --name starkbot starkbot

# Stop and remove
docker stop starkbot && docker rm starkbot
```

### Test the Container

```bash
# Health check
curl http://localhost:8080/health

# Open in browser
xdg-open http://localhost:8080  # Linux
open http://localhost:8080      # macOS
```

## Development with Hot Reload (Docker)

For active development, use the dev Docker configuration which provides automatic hot reloading for both frontend and backend changes.

### Start Development Environment

```bash
# Start with hot reload (first run will take longer to build)
docker compose -f docker-compose.dev.yml up --build

# Or run in background
docker compose -f docker-compose.dev.yml up --build -d

# View logs when running in background
docker compose -f docker-compose.dev.yml logs -f
```

### How Hot Reload Works

| Change Type | Behavior |
|-------------|----------|
| **Frontend** (HTML/CSS/JS in `stark-frontend/`) | Instant - files are volume-mounted directly |
| **Backend** (Rust code in `stark-backend/src/`) | Automatic recompilation via `cargo-watch` |

### Performance Notes

- First build is slower (compiling all dependencies)
- Subsequent rebuilds are fast thanks to cached dependencies
- Named volumes (`cargo-target`, `cargo-registry`) persist between restarts

### Stop Development Environment

```bash
# Stop containers
docker compose -f docker-compose.dev.yml down

# Stop and remove volumes (clean slate)
docker compose -f docker-compose.dev.yml down -v
```

## Deploy to DigitalOcean App Platform

### 1. Push to GitHub

```bash
git init
git add .
git commit -m "Initial commit"
git remote add origin git@github.com:yourusername/starkbot.git
git push -u origin main
```

### 2. Create App on DigitalOcean

1. Go to [DigitalOcean App Platform](https://cloud.digitalocean.com/apps)
2. Click **Create App**
3. Select **GitHub** and authorize access
4. Choose your `starkbot` repository
5. Select the branch (e.g., `main`)

### 3. Configure the App

DigitalOcean should auto-detect the Dockerfile. If not, manually configure:

- **Type**: Web Service
- **Source**: Dockerfile
- **HTTP Port**: 8080
- **Health Check Path**: `/health`

### 4. Set Environment Variables

In the App settings, add:

| Variable | Value |
|----------|-------|
| `SECRET_KEY` | *(generate with `openssl rand -base64 32`)* |
| `RUST_LOG` | `info` |
| `DATABASE_URL` | `/app/.db/stark.db` |

To encrypt the secret key:
1. Click on the `SECRET_KEY` variable
2. Check **Encrypt** to hide the value

### 5. Configure Persistent Storage (Optional)

For persistent SQLite data across deploys:

1. Go to **Components** > your web service > **Settings**
2. Under **Volumes**, click **Add Volume**
3. Set mount path to `/app/.db`
4. Update `DATABASE_URL` to `/app/.db/stark.db`

### 6. Deploy

Click **Create Resources** to deploy. The build takes a few minutes.

Your app will be available at: `https://your-app-name.ondigitalocean.app`

## App Spec (Alternative)

You can also deploy using a `.do/app.yaml` spec file:

```yaml
name: starkbot
services:
  - name: web
    dockerfile_path: Dockerfile
    github:
      repo: yourusername/starkbot
      branch: main
      deploy_on_push: true
    http_port: 8080
    health_check:
      http_path: /health
    instance_size_slug: basic-xxs
    instance_count: 1
    envs:
      - key: SECRET_KEY
        scope: RUN_TIME
        type: SECRET
      - key: RUST_LOG
        scope: RUN_TIME
        value: info
      - key: DATABASE_URL
        scope: RUN_TIME
        value: /app/.db/stark.db
```

Deploy with:
```bash
doctl apps create --spec .do/app.yaml
```

## Project Structure

```
starkbot/
├── Cargo.toml                 # Workspace manifest
├── Dockerfile                 # Production multi-stage build
├── Dockerfile.dev             # Development build with hot reload
├── docker-compose.yml         # Production Docker Compose
├── docker-compose.dev.yml     # Dev environment with volume mounts
├── stark-backend/             # Actix web server
│   └── src/
│       ├── main.rs            # Server entry point
│       ├── config.rs          # Environment config
│       ├── db/sqlite.rs       # SQLite + sessions
│       ├── controllers/       # API endpoints
│       └── middleware/        # Auth middleware
└── stark-frontend/            # Static frontend
    ├── index.html             # Login page
    ├── dashboard.html         # Protected dashboard
    ├── agent-chat.html        # Agent conversation interface
    ├── css/styles.css
    └── js/
```
