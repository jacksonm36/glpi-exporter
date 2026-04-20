# GLPI Server Updater

A robust, distro-agnostic Bash script that safely updates a self-hosted GLPI instance to the latest version. Features full auto-detection, plugin compatibility checks, backup/rollback failsafes, and idempotent multi-run support.

## Requirements

The script auto-detects and validates all dependencies at startup. At minimum you need:

- **Bash** >= 4.0
- **PHP CLI** (for `bin/console`)
- **curl** or **wget**
- **tar**, **gzip**
- **mysqldump** or **mariadb-dump**
- **mysql** or **mariadb** client
- **sudo** (script must run as root)

Optional tools that enhance functionality:
- `jq` -- cleaner GitHub API parsing (falls back to grep)
- `sha256sum` -- download integrity verification
- `systemctl` -- web server management
- `tput` -- colored terminal output

## Quick Start

```bash
# Basic usage -- auto-detect everything, update to latest
sudo bash glpi-update.sh

# Specify GLPI path explicitly
sudo bash glpi-update.sh --path /var/www/html/glpi

# Dry run -- see what would happen without making changes
sudo bash glpi-update.sh --dry-run

# Update to a specific version
sudo bash glpi-update.sh --target-version 11.0.6

# Backup only (no update)
sudo bash glpi-update.sh --backup-only
```

## How It Works

The script executes 10 sequential phases. If interrupted, it saves its progress and can resume from where it left off.

```
Phase 1:  Pre-flight checks (dependency + compatibility validation)
Phase 2:  Plugin compatibility check
Phase 3:  Full backup (database + files)
Phase 4:  Download and verify new release
Phase 5:  Enable maintenance mode
Phase 6:  Replace GLPI files (atomic swap)
Phase 7:  Database migration (php bin/console db:update)
Phase 8:  Plugin migration (10->11 special handling)
Phase 9:  Post-update health checks
Phase 10: Cleanup and final report
```

## Auto-Detection

The script automatically detects:

| Component | Detection Method |
|-----------|-----------------|
| GLPI path | Scans common locations, checks for `bin/console` |
| Data directories | Parses `downstream.php` and `local_define.php` for FHS overrides |
| GLPI version | CLI (`bin/console --version`), `define.php`, `version/` directory |
| Web server | Process detection (apache2/httpd/nginx), user from config |
| Database | Config from `config_db.php`, engine from client binary |
| Distro | `/etc/os-release` for install suggestions |

### Data Directory Detection

GLPI installations may store data in various locations. The script handles all layouts:

- **Simple**: Everything under the GLPI install directory
- **FHS-compliant**: Config in `/etc/glpi/`, data in `/var/lib/glpi/`, logs in `/var/log/glpi/`
- **Custom**: Any path defined in `local_define.php`

Override auto-detection with `--config-dir`, `--data-dir`, `--log-dir`.

## Failsafe Features

- **Lock file** prevents concurrent runs; stale locks are auto-cleared
- **State file** enables resume after crash or interruption
- **Automatic rollback** on failure during file replacement or database migration
- **Backup verification** checks archive integrity before proceeding
- **Dry-run mode** validates everything without making changes
- **Idempotent phases** check if work is already done before executing

## Plugin Handling

The script reads each plugin's `setup.php` to extract `MIN_GLPI_VERSION` and `MAX_GLPI_VERSION` constraints, then reports compatibility:

```
PLUGIN                    VERSION      MIN GLPI       MAX GLPI     STATUS
-------------------------------------------------------------------------
fusioninventory           10.0.6       10.0.0         10.0.99      INCOMPATIBLE
fields                    1.21.0       10.0.0         11.0.99      OK
formcreator               2.13.10      10.0.0         10.0.99      OK  [MIGRATE]
```

For 10.x to 11.x upgrades, the script automatically handles:
- `genericobject` -> core migration
- `formcreator` -> core migration
- `fields` -> ensures it stays enabled for FormCreator migration

## Configuration

Place `glpi-update.conf` next to the script or at `/etc/glpi-update.conf`. CLI arguments override config file values.

```bash
GLPI_PATH=""              # Auto-detected if empty
GLPI_CONFIG_DIR=""        # e.g. /etc/glpi
GLPI_VAR_DIR=""           # e.g. /var/lib/glpi
GLPI_LOG_DIR=""           # e.g. /var/log/glpi
BACKUP_DIR="/var/backups/glpi"
BACKUP_RETENTION=3
DOWNLOAD_TIMEOUT=300
DB_UPDATE_TIMEOUT=600
LOG_FILE="/var/log/glpi-update.log"
```

## All CLI Options

```
--path /path/to/glpi     GLPI installation path (auto-detected if omitted)
--config-dir /path       Override auto-detected GLPI config directory
--data-dir /path         Override auto-detected GLPI data/files directory
--log-dir /path          Override auto-detected GLPI log directory
--target-version X.Y.Z   Update to a specific version (latest if omitted)
--backup-dir /path       Backup directory (default: /var/backups/glpi)
--dry-run                Show what would happen without making changes
--backup-only            Only create a backup, do not update
--download-only          Only download the release tarball
--force                  Force update even if already on target version
--yes                    Skip all confirmation prompts
--no-plugin-check        Skip plugin compatibility checking
--no-rollback            Disable automatic rollback on failure
--verbose                Enable verbose/debug output
--help, -h               Show this help message
```

## Backup Structure

Each backup is stored in a timestamped subdirectory:

```
/var/backups/glpi/
  glpi_backup_20260409_143000/
    db_glpi_20260409_143000.sql.gz      # Full database dump
    glpi_files_20260409_143000.tar.gz   # Complete file archive
    backup_info.txt                      # Metadata
    quick_restore/
      downstream.php                     # Quick-access config files
      local_define.php
      config_db.php
```

Old backups are automatically pruned based on the retention policy (default: keep 3).

## Rollback

If the update fails during file replacement or database migration, the script automatically:

1. Restores the old GLPI directory from the `.old` backup
2. Drops and recreates the database
3. Restores the database from the backup dump
4. Disables maintenance mode

To disable automatic rollback: `--no-rollback`

## License

GNU General Public License v3.0 -- see [LICENSE](../LICENSE).
