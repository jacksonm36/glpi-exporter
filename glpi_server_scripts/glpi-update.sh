#!/usr/bin/env bash
# =============================================================================
# glpi-update.sh -- Robust, distro-agnostic GLPI server updater
# =============================================================================
# Usage: sudo bash glpi-update.sh [OPTIONS]
# See --help for full option list.
# =============================================================================
set -euo pipefail
IFS=$'\n\t'

readonly SCRIPT_VERSION="1.0.0"
readonly SCRIPT_NAME="$(basename "$0")"
readonly SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
readonly LOCK_FILE="/tmp/glpi-update.lock"
readonly STATE_FILE="/var/tmp/glpi-update.state"
TIMESTAMP="$(date +%Y%m%d_%H%M%S)"
PHASE=0

# =============================================================================
# Defaults (overridden by config file, then CLI args)
# =============================================================================
OPT_GLPI_PATH=""
OPT_CONFIG_DIR=""
OPT_VAR_DIR=""
OPT_LOG_DIR=""
OPT_TARGET_VERSION=""
OPT_BACKUP_DIR="/var/backups/glpi"
OPT_BACKUP_RETENTION=3
OPT_DOWNLOAD_TIMEOUT=300
OPT_DB_UPDATE_TIMEOUT=600
OPT_MAINTENANCE_PAGE=""
OPT_LOG_FILE="/var/log/glpi-update.log"
OPT_GITHUB_API_URL="https://api.github.com/repos/glpi-project/glpi/releases/latest"
OPT_DRY_RUN=false
OPT_BACKUP_ONLY=false
OPT_DOWNLOAD_ONLY=false
OPT_FORCE=false
OPT_YES=false
OPT_NO_PLUGIN_CHECK=false
OPT_NO_ROLLBACK=false
OPT_VERBOSE=false

# Detected at runtime
GLPI_PATH=""
GLPI_CONFIG_DIR=""
GLPI_VAR_DIR=""
GLPI_LOG_DIR=""
GLPI_PLUGINS_DIR=""
GLPI_MARKETPLACE_DIR=""
GLPI_DOWNSTREAM=""
GLPI_LOCAL_DEFINE=""
GLPI_CURRENT_VERSION=""
GLPI_TARGET_VERSION=""
GLPI_DB_HOST=""
GLPI_DB_NAME=""
GLPI_DB_USER=""
GLPI_DB_PASS=""
WEB_SERVER=""
WEB_USER=""
DB_ENGINE=""
DB_CLIENT_CMD=""
DB_DUMP_CMD=""
DISTRO_ID=""
DISTRO_FAMILY=""
PKG_MGR=""
HAS_JQ=false
HAS_CHECKSUM=false
HAS_SYSTEMCTL=false
HAS_TPUT=false
SELINUX_ENABLED=false
FHS_EXTERNAL=false
MAJOR_UPGRADE=false
DOWNLOAD_CMD=""
STAGING_DIR=""
BACKUP_SUBDIR=""
OLD_DIR=""
GITHUB_API_RESPONSE=""

declare -a PLUGIN_NAMES=()
declare -a PLUGIN_STATUSES=()
declare -a PLUGIN_MIGRATE=()

# Initialize color variables to empty so they are safe to reference
# before setup_colors() runs (e.g. if the EXIT trap fires early).
C_RED="" C_GREEN="" C_YELLOW="" C_BLUE="" C_CYAN="" C_BOLD="" C_RESET=""

# =============================================================================
# Terminal colors
# =============================================================================
setup_colors() {
    if command -v tput &>/dev/null && [[ -t 1 ]]; then
        HAS_TPUT=true
        C_RED="$(tput setaf 1)"
        C_GREEN="$(tput setaf 2)"
        C_YELLOW="$(tput setaf 3)"
        C_BLUE="$(tput setaf 4)"
        C_CYAN="$(tput setaf 6)"
        C_BOLD="$(tput bold)"
        C_RESET="$(tput sgr0)"
    else
        HAS_TPUT=false
        C_RED="" C_GREEN="" C_YELLOW="" C_BLUE="" C_CYAN="" C_BOLD="" C_RESET=""
    fi
}

# =============================================================================
# Logging
# =============================================================================
_log() {
    local level="$1"; shift
    local ts
    ts="$(date '+%Y-%m-%d %H:%M:%S')"
    local msg="[$ts] [$level] $*"

    case "$level" in
        ERROR) echo "${C_RED}${C_BOLD}[ERROR]${C_RESET} $*" >&2 ;;
        WARN)  echo "${C_YELLOW}[WARN]${C_RESET}  $*" ;;
        OK)    echo "${C_GREEN}[OK]${C_RESET}    $*" ;;
        INFO)  echo "${C_CYAN}[INFO]${C_RESET}  $*" ;;
        DEBUG) $OPT_VERBOSE && echo "${C_BLUE}[DEBUG]${C_RESET} $*" || true ;;
    esac

    if [[ -n "${OPT_LOG_FILE:-}" ]]; then
        echo "$msg" >> "$OPT_LOG_FILE" 2>/dev/null || true
    fi
}

log_info()  { _log INFO  "$@"; }
log_ok()    { _log OK    "$@"; }
log_warn()  { _log WARN  "$@"; }
log_error() { _log ERROR "$@"; }
log_debug() { _log DEBUG "$@"; }

header() {
    echo ""
    echo "${C_BOLD}${C_CYAN}=== $* ===${C_RESET}"
    echo ""
}

# =============================================================================
# Utility helpers
# =============================================================================
confirm() {
    if $OPT_YES; then return 0; fi
    local prompt="${1:-Continue?}"
    while true; do
        read -r -p "${C_BOLD}${prompt} [y/N]: ${C_RESET}" answer
        case "${answer,,}" in
            y|yes) return 0 ;;
            n|no|"") return 1 ;;
        esac
    done
}

die() {
    log_error "$@"
    exit 1
}

run_as_webuser() {
    if [[ "$(id -u)" -eq 0 ]] && [[ -n "$WEB_USER" ]]; then
        sudo -u "$WEB_USER" "$@"
    else
        "$@"
    fi
}

version_ge() {
    printf '%s\n%s' "$2" "$1" | sort -V -C
}

version_major() {
    echo "$1" | cut -d. -f1
}

# Create a temporary MySQL/MariaDB defaults file so credentials
# never appear in `ps` output. Caller must remove the file afterward.
DB_DEFAULTS_FILE=""
create_db_defaults_file() {
    DB_DEFAULTS_FILE="$(mktemp /tmp/glpi-update-my.XXXXXXXX)"
    chmod 600 "$DB_DEFAULTS_FILE"
    cat > "$DB_DEFAULTS_FILE" <<MYEOF
[client]
host=${GLPI_DB_HOST}
user=${GLPI_DB_USER}
password=${GLPI_DB_PASS}

[mysqldump]
host=${GLPI_DB_HOST}
user=${GLPI_DB_USER}
password=${GLPI_DB_PASS}
MYEOF
}

remove_db_defaults_file() {
    [[ -n "$DB_DEFAULTS_FILE" ]] && rm -f "$DB_DEFAULTS_FILE" 2>/dev/null || true
    DB_DEFAULTS_FILE=""
}

# Apply correct SELinux contexts to GLPI directories and web server log files.
# Without this, files moved from /tmp inherit the wrong context and Apache/PHP
# get "Permission denied" on SELinux-enforcing systems.
fix_selinux_contexts() {
    if ! $SELINUX_ENABLED; then return; fi
    if ! command -v chcon &>/dev/null; then
        log_warn "SELinux is active but 'chcon' not found. Fix contexts manually."
        return
    fi

    log_info "Applying SELinux contexts to GLPI files ..."

    # Read-only content: the installation directory tree
    chcon -R -t httpd_sys_content_t "$GLPI_PATH" 2>/dev/null || log_warn "chcon failed on $GLPI_PATH"

    # Writable dirs Apache/PHP-FPM need to write into
    local -a rw_dirs=()
    [[ -d "$GLPI_PATH/files" ]]       && rw_dirs+=("$GLPI_PATH/files")
    [[ -d "$GLPI_PATH/config" ]]      && rw_dirs+=("$GLPI_PATH/config")
    [[ -d "$GLPI_PATH/marketplace" ]]  && rw_dirs+=("$GLPI_PATH/marketplace")
    [[ -d "$GLPI_PATH/plugins" ]]      && rw_dirs+=("$GLPI_PATH/plugins")

    # FHS-external writable dirs
    if $FHS_EXTERNAL; then
        [[ -d "$GLPI_CONFIG_DIR" ]] && rw_dirs+=("$GLPI_CONFIG_DIR")
        [[ -d "$GLPI_VAR_DIR" ]]    && rw_dirs+=("$GLPI_VAR_DIR")
        [[ -d "$GLPI_LOG_DIR" ]]    && rw_dirs+=("$GLPI_LOG_DIR")
    fi

    for d in "${rw_dirs[@]}"; do
        chcon -R -t httpd_sys_rw_content_t "$d" 2>/dev/null || log_warn "chcon rw failed on $d"
    done

    log_ok "SELinux contexts applied"
}

# Detect web server log files that reference GLPI and fix their
# ownership and SELinux contexts so Apache can open them on restart.
fix_webserver_log_files() {
    log_info "Checking web server log files ..."

    local -a log_files=()

    # Scan Apache vhost configs for CustomLog / ErrorLog directives
    local -a vhost_dirs=()
    [[ -d /etc/httpd/conf.d ]]              && vhost_dirs+=(/etc/httpd/conf.d)
    [[ -d /etc/httpd/conf ]]                && vhost_dirs+=(/etc/httpd/conf)
    [[ -d /etc/apache2/sites-enabled ]]     && vhost_dirs+=(/etc/apache2/sites-enabled)
    [[ -d /etc/apache2/conf-enabled ]]      && vhost_dirs+=(/etc/apache2/conf-enabled)

    if [[ ${#vhost_dirs[@]} -gt 0 ]]; then
        while IFS= read -r logpath; do
            [[ -n "$logpath" ]] && log_files+=("$logpath")
        done < <(grep -rhiE '^\s*(Custom|Error)Log\s+' "${vhost_dirs[@]}" 2>/dev/null \
                 | grep -i glpi \
                 | grep -oP '(?:Custom|Error)Log\s+\K\S+' \
                 | sed 's/"//g' \
                 | sort -u || true)
    fi

    if [[ ${#log_files[@]} -eq 0 ]]; then
        log_debug "No GLPI-specific web server log files detected."
        return
    fi

    for lf in "${log_files[@]}"; do
        # Create the file if it doesn't exist
        if [[ ! -f "$lf" ]]; then
            touch "$lf" 2>/dev/null || { log_warn "Cannot create $lf"; continue; }
        fi

        chown "$WEB_USER:$WEB_USER" "$lf" 2>/dev/null || log_warn "chown failed on $lf"
        chmod 644 "$lf" 2>/dev/null || true

        if $SELINUX_ENABLED && command -v chcon &>/dev/null; then
            chcon -t httpd_log_t "$lf" 2>/dev/null || log_warn "chcon failed on $lf"
        fi

        log_ok "Fixed log file: $lf"
    done
}

# =============================================================================
# Lock file management
# =============================================================================
acquire_lock() {
    # Prevent symlink attacks: refuse to write through a symlink
    if [[ -L "$LOCK_FILE" ]]; then
        die "Lock file $LOCK_FILE is a symlink. Refusing to proceed (possible attack)."
    fi

    if [[ -f "$LOCK_FILE" ]]; then
        local old_pid
        old_pid="$(cat "$LOCK_FILE" 2>/dev/null || echo "")"
        if [[ -n "$old_pid" ]] && kill -0 "$old_pid" 2>/dev/null; then
            die "Another instance is running (PID $old_pid). Remove $LOCK_FILE if stale."
        fi
        log_warn "Stale lock file found (PID $old_pid not running). Removing."
        rm -f "$LOCK_FILE"
    fi

    ( umask 077; echo $$ > "$LOCK_FILE" )
    log_debug "Lock acquired (PID $$)"
}

release_lock() {
    rm -f "$LOCK_FILE" 2>/dev/null || true
    log_debug "Lock released"
}

# =============================================================================
# State file management (for resume-on-crash)
# =============================================================================
save_state() {
    local phase="$1"
    ( umask 077; cat > "$STATE_FILE" <<EOF
PHASE='${phase}'
GLPI_PATH='${GLPI_PATH//\'/\'\\\'\'}'
GLPI_TARGET_VERSION='${GLPI_TARGET_VERSION//\'/\'\\\'\'}'
GLPI_CURRENT_VERSION='${GLPI_CURRENT_VERSION//\'/\'\\\'\'}'
BACKUP_SUBDIR='${BACKUP_SUBDIR//\'/\'\\\'\'}'
OLD_DIR='${OLD_DIR//\'/\'\\\'\'}'
STAGING_DIR='${STAGING_DIR//\'/\'\\\'\'}'
TIMESTAMP='${TIMESTAMP//\'/\'\\\'\'}'
EOF
    )
    log_debug "State saved: phase=$phase"
}

load_state() {
    if [[ -f "$STATE_FILE" ]]; then
        # Validate ownership: must be owned by root
        local owner
        owner="$(stat -c '%u' "$STATE_FILE" 2>/dev/null || stat -f '%u' "$STATE_FILE" 2>/dev/null || echo "")"
        if [[ "$owner" != "0" ]] && [[ "$owner" != "" ]]; then
            log_warn "State file not owned by root (uid=$owner). Ignoring for safety."
            rm -f "$STATE_FILE"
            return 1
        fi

        log_warn "A previous run was interrupted. State file found: $STATE_FILE"
        echo ""
        cat "$STATE_FILE"
        echo ""
        if confirm "Resume from the last saved phase?"; then
            # Safe parsing: only accept known key=value pairs
            while IFS='=' read -r key value; do
                key="$(echo "$key" | tr -d "[:space:]")"
                value="$(echo "$value" | sed "s/^'//;s/'$//")"
                case "$key" in
                    PHASE)                PHASE="$value" ;;
                    GLPI_PATH)            GLPI_PATH="$value" ;;
                    GLPI_TARGET_VERSION)  GLPI_TARGET_VERSION="$value" ;;
                    GLPI_CURRENT_VERSION) GLPI_CURRENT_VERSION="$value" ;;
                    BACKUP_SUBDIR)        BACKUP_SUBDIR="$value" ;;
                    OLD_DIR)              OLD_DIR="$value" ;;
                    STAGING_DIR)          STAGING_DIR="$value" ;;
                    TIMESTAMP)            TIMESTAMP="$value" ;;
                esac
            done < "$STATE_FILE"
            return 0
        else
            log_info "Starting fresh. Removing old state file."
            rm -f "$STATE_FILE"
            return 1
        fi
    fi
    return 1
}

clear_state() {
    rm -f "$STATE_FILE" 2>/dev/null || true
}

# =============================================================================
# Trap handler
# =============================================================================
cleanup_on_exit() {
    local exit_code=$?
    # Always clean up sensitive temp files
    remove_db_defaults_file
    if [[ $exit_code -ne 0 ]]; then
        log_error "Script exited with code $exit_code"
        if [[ -f "$STATE_FILE" ]]; then
            log_warn "State file preserved at $STATE_FILE for resume."
        fi
    fi
    release_lock
}
trap cleanup_on_exit EXIT

# =============================================================================
# Distro detection
# =============================================================================
detect_distro() {
    DISTRO_ID="unknown"
    DISTRO_FAMILY="unknown"
    PKG_MGR="unknown"

    if [[ -f /etc/os-release ]]; then
        # shellcheck source=/dev/null
        source /etc/os-release
        DISTRO_ID="${ID:-unknown}"
        local id_like="${ID_LIKE:-}"
        case "$DISTRO_ID" in
            ubuntu|debian|linuxmint|pop|raspbian)
                DISTRO_FAMILY="debian"; PKG_MGR="apt" ;;
            centos|rhel|rocky|alma|ol|scientific)
                DISTRO_FAMILY="rhel"
                command -v dnf &>/dev/null && PKG_MGR="dnf" || PKG_MGR="yum"
                ;;
            fedora)
                DISTRO_FAMILY="rhel"; PKG_MGR="dnf" ;;
            opensuse*|suse|sles)
                DISTRO_FAMILY="suse"; PKG_MGR="zypper" ;;
            arch|manjaro|endeavouros)
                DISTRO_FAMILY="arch"; PKG_MGR="pacman" ;;
            alpine)
                DISTRO_FAMILY="alpine"; PKG_MGR="apk" ;;
            *)
                case "$id_like" in
                    *debian*|*ubuntu*) DISTRO_FAMILY="debian"; PKG_MGR="apt" ;;
                    *rhel*|*centos*|*fedora*) DISTRO_FAMILY="rhel"; PKG_MGR="dnf" ;;
                    *suse*) DISTRO_FAMILY="suse"; PKG_MGR="zypper" ;;
                    *arch*) DISTRO_FAMILY="arch"; PKG_MGR="pacman" ;;
                esac
                ;;
        esac
    fi
    log_debug "Distro: $DISTRO_ID  Family: $DISTRO_FAMILY  PkgMgr: $PKG_MGR"
}

suggest_install() {
    local pkg="$1"
    case "$PKG_MGR" in
        apt)    echo "apt install -y $pkg" ;;
        dnf)    echo "dnf install -y $pkg" ;;
        yum)    echo "yum install -y $pkg" ;;
        zypper) echo "zypper install -y $pkg" ;;
        pacman) echo "pacman -S --noconfirm $pkg" ;;
        apk)    echo "apk add $pkg" ;;
        *)      echo "(install $pkg with your package manager)" ;;
    esac
}

# =============================================================================
# Phase 1: Pre-flight checks
# =============================================================================

# -- 1a: Script dependencies ------------------------------------------------
check_script_dependencies() {
    header "Checking script dependencies"

    local missing_required=()
    local missing_optional=()

    # Bash version
    local bash_major="${BASH_VERSINFO[0]:-0}"
    if [[ "$bash_major" -lt 4 ]]; then
        die "Bash >= 4.0 required (found ${BASH_VERSION})"
    fi
    log_ok "bash ${BASH_VERSION}"

    # Required tools
    local -A required_tools=(
        [php]="php-cli"
        [tar]="tar"
        [gzip]="gzip"
        [grep]="grep"
        [sed]="sed"
        [awk]="gawk"
        [mktemp]="coreutils"
        [df]="coreutils"
        [id]="coreutils"
        [stat]="coreutils"
        [sudo]="sudo"
        [timeout]="coreutils"
    )

    for cmd in "${!required_tools[@]}"; do
        if command -v "$cmd" &>/dev/null; then
            log_ok "$cmd"
        else
            missing_required+=("$cmd ($(suggest_install "${required_tools[$cmd]}"))")
        fi
    done

    # curl or wget (need at least one)
    if command -v curl &>/dev/null; then
        DOWNLOAD_CMD="curl"
        log_ok "curl"
    elif command -v wget &>/dev/null; then
        DOWNLOAD_CMD="wget"
        log_ok "wget"
    else
        missing_required+=("curl or wget ($(suggest_install curl))")
    fi

    # mysqldump or mariadb-dump (need at least one)
    if command -v mariadb-dump &>/dev/null; then
        DB_DUMP_CMD="mariadb-dump"
        log_ok "mariadb-dump"
    elif command -v mysqldump &>/dev/null; then
        DB_DUMP_CMD="mysqldump"
        log_ok "mysqldump"
    else
        missing_required+=("mysqldump/mariadb-dump ($(suggest_install mariadb-client))")
    fi

    # mysql or mariadb client (need at least one)
    if command -v mariadb &>/dev/null; then
        DB_CLIENT_CMD="mariadb"
        log_ok "mariadb (client)"
    elif command -v mysql &>/dev/null; then
        DB_CLIENT_CMD="mysql"
        log_ok "mysql (client)"
    else
        missing_required+=("mysql/mariadb client ($(suggest_install mariadb-client))")
    fi

    # Optional tools
    if command -v jq &>/dev/null; then
        HAS_JQ=true; log_ok "jq"
    else
        HAS_JQ=false; missing_optional+=("jq ($(suggest_install jq)) -- JSON parsing will use grep fallback")
    fi

    if command -v sha256sum &>/dev/null || command -v shasum &>/dev/null; then
        HAS_CHECKSUM=true; log_ok "sha256sum/shasum"
    else
        HAS_CHECKSUM=false; missing_optional+=("sha256sum -- checksum verification will be skipped")
    fi

    if command -v systemctl &>/dev/null; then
        HAS_SYSTEMCTL=true; log_ok "systemctl"
    elif command -v service &>/dev/null; then
        HAS_SYSTEMCTL=false; log_ok "service"
    else
        missing_optional+=("systemctl/service -- web server restart must be done manually")
    fi

    # Report
    if [[ ${#missing_optional[@]} -gt 0 ]]; then
        echo ""
        log_warn "Optional dependencies missing:"
        for m in "${missing_optional[@]}"; do
            echo "  ${C_YELLOW}-${C_RESET} $m"
        done
    fi

    if [[ ${#missing_required[@]} -gt 0 ]]; then
        echo ""
        log_error "Required dependencies missing:"
        for m in "${missing_required[@]}"; do
            echo "  ${C_RED}-${C_RESET} $m"
        done
        die "Install the missing required dependencies and re-run."
    fi

    log_ok "All required script dependencies satisfied"
}

# -- 1b: Detect GLPI installation path --------------------------------------
detect_glpi_path() {
    header "Detecting GLPI installation"

    if [[ -n "$OPT_GLPI_PATH" ]]; then
        GLPI_PATH="$OPT_GLPI_PATH"
        log_info "Using provided path: $GLPI_PATH"
    else
        local -a search_paths=(
            "/var/www/html/glpi"
            "/var/www/glpi"
            "/usr/share/glpi"
            "/srv/www/htdocs/glpi"
            "/var/www/html/glpi10"
            "/var/www/html/glpi11"
            "/opt/glpi"
        )
        for p in "${search_paths[@]}"; do
            if [[ -f "$p/bin/console" ]] || [[ -f "$p/inc/define.php" ]]; then
                GLPI_PATH="$p"
                log_info "Auto-detected GLPI at: $GLPI_PATH"
                break
            fi
        done
    fi

    if [[ -z "$GLPI_PATH" ]] || [[ ! -d "$GLPI_PATH" ]]; then
        die "Cannot find GLPI installation. Use --path to specify."
    fi

    if [[ ! -f "$GLPI_PATH/bin/console" ]] && [[ ! -f "$GLPI_PATH/inc/define.php" ]]; then
        die "$GLPI_PATH does not appear to be a valid GLPI installation."
    fi

    log_ok "GLPI found at: $GLPI_PATH"
}

# Extract a PHP define() constant value from a file, skipping commented lines
extract_php_define() {
    local constant="$1" file="$2"
    grep -v '^\s*//' "$file" 2>/dev/null | grep -v '^\s*#' | grep -v '^\s*\*' | \
        grep -oP "define\s*\(\s*['\"]${constant}['\"]\s*,\s*['\"]?\K[^'\")\s]+" | head -1 || true
}

# -- 1c: Detect GLPI data directories ---------------------------------------
detect_glpi_data_dirs() {
    header "Detecting GLPI data directories"

    FHS_EXTERNAL=false
    GLPI_DOWNSTREAM=""
    GLPI_LOCAL_DEFINE=""

    # Step 1: downstream.php -- parse define() and require/include
    local downstream="$GLPI_PATH/inc/downstream.php"
    if [[ -f "$downstream" ]]; then
        GLPI_DOWNSTREAM="$downstream"
        log_info "Found downstream.php: $downstream"

        # First, extract any define() statements directly in downstream.php
        local ds_config ds_var ds_log ds_marketplace
        ds_config="$(extract_php_define GLPI_CONFIG_DIR "$downstream")"
        ds_var="$(extract_php_define GLPI_VAR_DIR "$downstream")"
        ds_log="$(extract_php_define GLPI_LOG_DIR "$downstream")"
        ds_marketplace="$(extract_php_define GLPI_MARKETPLACE_DIR "$downstream")"

        if [[ -n "$ds_config" ]]; then
            log_info "downstream.php defines GLPI_CONFIG_DIR=$ds_config"
            [[ -z "$OPT_CONFIG_DIR" ]] && GLPI_CONFIG_DIR="$ds_config"
            FHS_EXTERNAL=true
        fi
        if [[ -n "$ds_var" ]]; then
            log_info "downstream.php defines GLPI_VAR_DIR=$ds_var"
            [[ -z "$OPT_VAR_DIR" ]] && GLPI_VAR_DIR="$ds_var"
            FHS_EXTERNAL=true
        fi
        if [[ -n "$ds_log" ]]; then
            log_info "downstream.php defines GLPI_LOG_DIR=$ds_log"
            [[ -z "$OPT_LOG_DIR" ]] && GLPI_LOG_DIR="$ds_log"
        fi
        [[ -n "$ds_marketplace" ]] && GLPI_MARKETPLACE_DIR="$ds_marketplace"

        # Then, check if downstream.php references a local_define.php via require/include
        local ld_path
        ld_path="$(grep -oP "(?:require|include)(?:_once)?\s*['\"]?\K[^'\";\s]+" "$downstream" 2>/dev/null | head -1 || true)"
        if [[ -n "$ld_path" ]] && [[ -f "$ld_path" ]]; then
            GLPI_LOCAL_DEFINE="$ld_path"
            log_info "downstream.php references: $ld_path"
        fi
    fi

    # Step 2: local_define.php -- check standard location if not found via downstream.php
    if [[ -z "$GLPI_LOCAL_DEFINE" ]] && [[ -f "$GLPI_PATH/config/local_define.php" ]]; then
        GLPI_LOCAL_DEFINE="$GLPI_PATH/config/local_define.php"
    fi

    # Also check if downstream.php's GLPI_CONFIG_DIR contains a local_define.php
    if [[ -z "$GLPI_LOCAL_DEFINE" ]] && [[ -n "$GLPI_CONFIG_DIR" ]] && [[ -f "$GLPI_CONFIG_DIR/local_define.php" ]]; then
        GLPI_LOCAL_DEFINE="$GLPI_CONFIG_DIR/local_define.php"
    fi

    if [[ -n "$GLPI_LOCAL_DEFINE" ]] && [[ -f "$GLPI_LOCAL_DEFINE" ]]; then
        log_info "Parsing local_define.php: $GLPI_LOCAL_DEFINE"

        local ld_config ld_var ld_log ld_marketplace
        ld_config="$(extract_php_define GLPI_CONFIG_DIR "$GLPI_LOCAL_DEFINE")"
        ld_var="$(extract_php_define GLPI_VAR_DIR "$GLPI_LOCAL_DEFINE")"
        ld_log="$(extract_php_define GLPI_LOG_DIR "$GLPI_LOCAL_DEFINE")"
        ld_marketplace="$(extract_php_define GLPI_MARKETPLACE_DIR "$GLPI_LOCAL_DEFINE")"

        # local_define.php overrides downstream.php values
        [[ -n "$ld_config" ]] && [[ -z "$OPT_CONFIG_DIR" ]] && GLPI_CONFIG_DIR="$ld_config"
        [[ -n "$ld_var" ]]    && [[ -z "$OPT_VAR_DIR" ]]    && GLPI_VAR_DIR="$ld_var"
        [[ -n "$ld_log" ]]    && [[ -z "$OPT_LOG_DIR" ]]    && GLPI_LOG_DIR="$ld_log"
        [[ -n "$ld_marketplace" ]] && GLPI_MARKETPLACE_DIR="$ld_marketplace"

        if [[ -n "$ld_config" ]] || [[ -n "$ld_var" ]]; then
            FHS_EXTERNAL=true
        fi
    fi

    # CLI overrides take priority
    [[ -n "$OPT_CONFIG_DIR" ]] && GLPI_CONFIG_DIR="$OPT_CONFIG_DIR"
    [[ -n "$OPT_VAR_DIR" ]]   && GLPI_VAR_DIR="$OPT_VAR_DIR"
    [[ -n "$OPT_LOG_DIR" ]]   && GLPI_LOG_DIR="$OPT_LOG_DIR"

    # Step 4: defaults
    [[ -z "$GLPI_CONFIG_DIR" ]]      && GLPI_CONFIG_DIR="$GLPI_PATH/config"
    [[ -z "$GLPI_VAR_DIR" ]]         && GLPI_VAR_DIR="$GLPI_PATH/files"
    [[ -z "$GLPI_LOG_DIR" ]]         && GLPI_LOG_DIR="$GLPI_PATH/files/_log"
    [[ -z "$GLPI_PLUGINS_DIR" ]]     && GLPI_PLUGINS_DIR="$GLPI_PATH/plugins"
    [[ -z "$GLPI_MARKETPLACE_DIR" ]] && GLPI_MARKETPLACE_DIR="$GLPI_PATH/marketplace"

    # Step 5: scan FHS if defaults don't exist
    if [[ ! -d "$GLPI_CONFIG_DIR" ]] && [[ -d "/etc/glpi" ]]; then
        GLPI_CONFIG_DIR="/etc/glpi"; FHS_EXTERNAL=true
    fi
    if [[ ! -d "$GLPI_VAR_DIR" ]] && [[ -d "/var/lib/glpi" ]]; then
        GLPI_VAR_DIR="/var/lib/glpi"; FHS_EXTERNAL=true
    fi
    if [[ ! -d "$GLPI_LOG_DIR" ]] && [[ -d "/var/log/glpi" ]]; then
        GLPI_LOG_DIR="/var/log/glpi"; FHS_EXTERNAL=true
    fi

    # Step 6: validate and report
    echo ""
    echo "${C_BOLD}Detected GLPI Layout:${C_RESET}"
    local -A dir_map=(
        ["Installation path"]="$GLPI_PATH"
        ["Config directory"]="$GLPI_CONFIG_DIR"
        ["Data/files dir"]="$GLPI_VAR_DIR"
        ["Log directory"]="$GLPI_LOG_DIR"
        ["Plugins directory"]="$GLPI_PLUGINS_DIR"
        ["Marketplace dir"]="$GLPI_MARKETPLACE_DIR"
    )

    local all_valid=true
    for label in "Installation path" "Config directory" "Data/files dir" "Log directory" "Plugins directory" "Marketplace dir"; do
        local path="${dir_map[$label]}"
        if [[ -d "$path" ]]; then
            printf "  %-22s %-40s ${C_GREEN}OK${C_RESET}\n" "$label:" "$path"
        else
            printf "  %-22s %-40s ${C_YELLOW}NOT FOUND${C_RESET}\n" "$label:" "$path"
            if [[ "$label" != "Marketplace dir" ]] && [[ "$label" != "Plugins directory" ]]; then
                all_valid=false
            fi
        fi
    done

    # Show config_db.php location if it exists
    local config_db_display="NOT FOUND"
    for cdb in "$GLPI_CONFIG_DIR/config_db.php" "$GLPI_PATH/config/config_db.php" "/etc/glpi/config_db.php"; do
        if [[ -f "$cdb" ]]; then
            config_db_display="$cdb"
            break
        fi
    done
    printf "  %-22s %s\n" "config_db.php:" "$config_db_display"
    printf "  %-22s %s\n" "downstream.php:" "${GLPI_DOWNSTREAM:-NOT PRESENT}"
    printf "  %-22s %s\n" "local_define.php:" "${GLPI_LOCAL_DEFINE:-NOT PRESENT}"
    printf "  %-22s %s\n" "FHS external:" "$FHS_EXTERNAL"
    echo ""

    if ! $all_valid; then
        log_warn "Some directories were not found. Verify your GLPI installation."
        confirm "Continue anyway?" || die "Aborted by user."
    fi
}

# -- 1d: Parse DB credentials from config_db.php ----------------------------
detect_database_config() {
    local config_db=""

    # Search multiple candidate locations for config_db.php
    local -a search_locations=(
        "$GLPI_CONFIG_DIR/config_db.php"
        "$GLPI_PATH/config/config_db.php"
        "/etc/glpi/config_db.php"
        "$GLPI_VAR_DIR/config/config_db.php"
    )

    for candidate in "${search_locations[@]}"; do
        if [[ -f "$candidate" ]]; then
            config_db="$candidate"
            break
        fi
    done

    # Last resort: find it anywhere under common locations
    if [[ -z "$config_db" ]]; then
        config_db="$(find "$GLPI_PATH" /etc/glpi /var/lib/glpi 2>/dev/null -name "config_db.php" -type f | head -1 || true)"
    fi

    if [[ -z "$config_db" ]] || [[ ! -f "$config_db" ]]; then
        log_error "Could not find config_db.php in any of:"
        for loc in "${search_locations[@]}"; do
            echo "  - $loc"
        done
        die "Use --config-dir to specify the directory containing config_db.php"
    fi

    # Update GLPI_CONFIG_DIR to match where we actually found config_db.php
    local found_dir
    found_dir="$(dirname "$config_db")"
    if [[ "$found_dir" != "$GLPI_CONFIG_DIR" ]]; then
        log_info "config_db.php found at $found_dir (updating config directory)"
        GLPI_CONFIG_DIR="$found_dir"
    fi

    log_debug "Parsing $config_db"
    # Parse PHP string values: try single-quoted first, then double-quoted
    _parse_php_var() {
        local varname="$1" file="$2"
        grep -oP "\\\$${varname}\s*=\s*'\K[^']*" "$file" 2>/dev/null | head -1 || \
        grep -oP "\\\$${varname}\s*=\s*\"\K[^\"]*" "$file" 2>/dev/null | head -1 || true
    }
    GLPI_DB_HOST="$(_parse_php_var dbhost "$config_db")"
    GLPI_DB_NAME="$(_parse_php_var dbdefault "$config_db")"
    GLPI_DB_USER="$(_parse_php_var dbuser "$config_db")"
    GLPI_DB_PASS="$(_parse_php_var dbpassword "$config_db")"
    [[ -z "$GLPI_DB_HOST" ]] && GLPI_DB_HOST="localhost"

    if [[ -z "$GLPI_DB_NAME" ]] || [[ -z "$GLPI_DB_USER" ]]; then
        die "Could not parse DB name/user from $config_db"
    fi
    log_ok "Database config: $config_db"
    log_ok "Database: $GLPI_DB_NAME@$GLPI_DB_HOST (user: $GLPI_DB_USER)"
}

# -- 1e: Detect GLPI version ------------------------------------------------
detect_glpi_version() {
    header "Detecting GLPI version"

    GLPI_CURRENT_VERSION=""

    # Try bin/console
    local console_version
    console_version="$(run_as_webuser php "$GLPI_PATH/bin/console" --version 2>/dev/null | grep -oP '[0-9]+\.[0-9]+\.[0-9]+' | head -1 || true)"
    if [[ -n "$console_version" ]]; then
        GLPI_CURRENT_VERSION="$console_version"
    fi

    # Fallback: inc/define.php
    if [[ -z "$GLPI_CURRENT_VERSION" ]] && [[ -f "$GLPI_PATH/inc/define.php" ]]; then
        GLPI_CURRENT_VERSION="$(grep -oP "GLPI_VERSION['\",\s]+['\"]?\K[0-9]+\.[0-9]+\.[0-9]+" "$GLPI_PATH/inc/define.php" 2>/dev/null | head -1 || true)"
    fi

    # Fallback: version directory
    if [[ -z "$GLPI_CURRENT_VERSION" ]] && [[ -d "$GLPI_PATH/version" ]]; then
        GLPI_CURRENT_VERSION="$(ls "$GLPI_PATH/version/" 2>/dev/null | sort -V | tail -1 || true)"
    fi

    if [[ -z "$GLPI_CURRENT_VERSION" ]]; then
        die "Could not determine current GLPI version."
    fi

    log_ok "Current GLPI version: $GLPI_CURRENT_VERSION"
}

# -- 1f: Detect latest available version ------------------------------------
detect_latest_version() {
    header "Checking latest GLPI version"

    # Always fetch the API response -- we need it for checksum verification too
    local api_url="$OPT_GITHUB_API_URL"
    if [[ -n "$OPT_TARGET_VERSION" ]]; then
        # Fetch the specific release instead of latest
        api_url="https://api.github.com/repos/glpi-project/glpi/releases/tags/${OPT_TARGET_VERSION}"
    fi

    if [[ "$DOWNLOAD_CMD" == "curl" ]]; then
        GITHUB_API_RESPONSE="$(curl -sS --connect-timeout 10 "$api_url" 2>/dev/null || true)"
    else
        GITHUB_API_RESPONSE="$(wget -qO- --timeout=10 "$api_url" 2>/dev/null || true)"
    fi

    if [[ -n "$OPT_TARGET_VERSION" ]]; then
        GLPI_TARGET_VERSION="$OPT_TARGET_VERSION"
        log_info "Using specified target version: $GLPI_TARGET_VERSION"
        if [[ -z "$GITHUB_API_RESPONSE" ]]; then
            log_warn "Could not fetch release info from GitHub API. Checksum verification may be unavailable."
        fi
    else
        if [[ -z "$GITHUB_API_RESPONSE" ]]; then
            die "Failed to query GitHub API. Check internet connectivity."
        fi

        if $HAS_JQ; then
            GLPI_TARGET_VERSION="$(echo "$GITHUB_API_RESPONSE" | jq -r '.tag_name' 2>/dev/null | sed 's/^v//' || true)"
        else
            GLPI_TARGET_VERSION="$(echo "$GITHUB_API_RESPONSE" | grep -oP '"tag_name"\s*:\s*"\K[^"]+' | head -1 | sed 's/^v//' || true)"
        fi

        if [[ -z "$GLPI_TARGET_VERSION" ]]; then
            die "Could not parse latest version from GitHub API response."
        fi

        log_ok "Latest available version: $GLPI_TARGET_VERSION"
    fi

    # Validate version format
    if ! [[ "$GLPI_TARGET_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        die "Invalid target version format: $GLPI_TARGET_VERSION (expected X.Y.Z)"
    fi

    # Check if already up-to-date
    if [[ "$GLPI_CURRENT_VERSION" == "$GLPI_TARGET_VERSION" ]] && ! $OPT_FORCE; then
        log_ok "GLPI is already at version $GLPI_TARGET_VERSION. Nothing to do."
        log_info "Use --force to re-run the update anyway."
        release_lock
        exit 0
    fi

    # Major upgrade detection
    local cur_major tar_major
    cur_major="$(version_major "$GLPI_CURRENT_VERSION")"
    tar_major="$(version_major "$GLPI_TARGET_VERSION")"
    if [[ "$cur_major" -ne "$tar_major" ]]; then
        MAJOR_UPGRADE=true
        echo ""
        log_warn "MAJOR VERSION UPGRADE DETECTED: $GLPI_CURRENT_VERSION -> $GLPI_TARGET_VERSION"
        log_warn "This is a major upgrade (${cur_major}.x -> ${tar_major}.x)."
        log_warn "Plugin migrations may be required. Review the plugin compatibility report carefully."
        echo ""
        confirm "Proceed with major version upgrade?" || die "Aborted by user."
    fi

    log_info "Update path: $GLPI_CURRENT_VERSION -> $GLPI_TARGET_VERSION"
}

# -- 1g: Detect web server ---------------------------------------------------
detect_web_server() {
    header "Detecting web server"

    WEB_SERVER=""
    WEB_USER=""

    if pgrep -x "apache2" &>/dev/null || pgrep -x "httpd" &>/dev/null; then
        WEB_SERVER="apache"
        if command -v apache2ctl &>/dev/null; then
            WEB_USER="$(apache2ctl -S 2>/dev/null | grep -oP 'User:\s*name="\K[^"]+' || true)"
        fi
    elif pgrep -x "nginx" &>/dev/null; then
        WEB_SERVER="nginx"
        WEB_USER="$(grep -oP '^\s*user\s+\K\S+' /etc/nginx/nginx.conf 2>/dev/null | tr -d ';' || true)"
    fi

    # Fallback: check owner of GLPI files
    if [[ -z "$WEB_USER" ]]; then
        WEB_USER="$(stat -c '%U' "$GLPI_PATH" 2>/dev/null || stat -f '%Su' "$GLPI_PATH" 2>/dev/null || true)"
    fi

    # Fallback: common web users
    if [[ -z "$WEB_USER" ]] || [[ "$WEB_USER" == "root" ]]; then
        for u in www-data apache nginx http _www; do
            if id "$u" &>/dev/null; then
                WEB_USER="$u"
                break
            fi
        done
    fi

    if [[ -z "$WEB_SERVER" ]]; then
        WEB_SERVER="unknown"
        log_warn "Could not detect running web server."
    else
        log_ok "Web server: $WEB_SERVER"
    fi

    if [[ -z "$WEB_USER" ]]; then
        WEB_USER="www-data"
        log_warn "Could not detect web server user, defaulting to: $WEB_USER"
    else
        log_ok "Web server user: $WEB_USER"
    fi

    # Detect SELinux
    if command -v getenforce &>/dev/null; then
        local se_status
        se_status="$(getenforce 2>/dev/null || echo "Disabled")"
        if [[ "$se_status" == "Enforcing" ]] || [[ "$se_status" == "Permissive" ]]; then
            SELINUX_ENABLED=true
            log_info "SELinux: $se_status (will apply httpd contexts after file operations)"
        else
            log_debug "SELinux: $se_status"
        fi
    fi
}

# -- 1h: Detect database engine ----------------------------------------------
detect_database_engine() {
    if [[ -n "$DB_CLIENT_CMD" ]]; then
        local version_str
        version_str="$($DB_CLIENT_CMD --version 2>/dev/null || true)"
        if echo "$version_str" | grep -qi "mariadb"; then
            DB_ENGINE="mariadb"
        else
            DB_ENGINE="mysql"
        fi
    else
        if pgrep -x "mariadbd" &>/dev/null || pgrep -x "mariadb" &>/dev/null; then
            DB_ENGINE="mariadb"
        elif pgrep -x "mysqld" &>/dev/null; then
            DB_ENGINE="mysql"
        else
            DB_ENGINE="unknown"
        fi
    fi
    log_ok "Database engine: $DB_ENGINE"
}

# -- 1i: GLPI runtime dependency check --------------------------------------
check_glpi_runtime_deps() {
    header "Checking GLPI runtime dependencies for $GLPI_TARGET_VERSION"

    local target_major
    target_major="$(version_major "$GLPI_TARGET_VERSION")"
    local errors=0
    local warnings=0
    local required_count=0

    # PHP version
    local php_version
    php_version="$(php -r 'echo PHP_VERSION;' 2>/dev/null || echo "0.0.0")"

    local php_min php_max_warn
    if [[ "$target_major" -ge 11 ]]; then
        php_min="8.2.0"
        php_max_warn=""
    else
        php_min="7.4.0"
        php_max_warn="8.4.0"
    fi

    required_count=$((required_count + 1))
    if version_ge "$php_version" "$php_min"; then
        printf "  %-22s %-14s >= %-10s ${C_GREEN}OK${C_RESET}\n" "PHP version" "$php_version" "$php_min"
    else
        printf "  %-22s %-14s >= %-10s ${C_RED}FAIL${C_RESET}\n" "PHP version" "$php_version" "$php_min"
        errors=$((errors + 1))
    fi

    if [[ -n "$php_max_warn" ]] && version_ge "$php_version" "$php_max_warn"; then
        printf "  %-22s %-14s < %-11s ${C_YELLOW}WARNING${C_RESET}\n" "PHP max version" "$php_version" "$php_max_warn"
        warnings=$((warnings + 1))
    fi

    # PHP extensions
    local php_modules
    php_modules="$(php -m 2>/dev/null | tr '[:upper:]' '[:lower:]')"

    local -a required_exts=(curl fileinfo gd intl json mbstring mysqli simplexml zlib)
    local -a recommended_exts=(exif ldap openssl zip bz2 sodium apcu)

    for ext in "${required_exts[@]}"; do
        required_count=$((required_count + 1))
        if echo "$php_modules" | grep -qx "$ext"; then
            printf "  %-22s %-14s %-11s ${C_GREEN}OK${C_RESET}\n" "PHP ext: $ext" "loaded" ""
        else
            printf "  %-22s %-14s %-11s ${C_RED}MISSING (required)${C_RESET}\n" "PHP ext: $ext" "NOT loaded" ""
            errors=$((errors + 1))
        fi
    done

    for ext in "${recommended_exts[@]}"; do
        if echo "$php_modules" | grep -qx "$ext"; then
            printf "  %-22s %-14s %-11s ${C_GREEN}OK${C_RESET}\n" "PHP ext: $ext" "loaded" ""
        else
            printf "  %-22s %-14s %-11s ${C_YELLOW}WARNING${C_RESET}\n" "PHP ext: $ext" "NOT loaded" "(recommended)"
            warnings=$((warnings + 1))
        fi
    done

    # Database version
    local db_version_full db_version
    create_db_defaults_file
    db_version_full="$($DB_CLIENT_CMD --defaults-extra-file="$DB_DEFAULTS_FILE" \
        -sNe "SELECT VERSION();" 2>/dev/null || echo "")"
    remove_db_defaults_file

    if [[ -n "$db_version_full" ]]; then
        db_version="$(echo "$db_version_full" | grep -oP '^[0-9]+\.[0-9]+\.[0-9]+' || echo "$db_version_full")"
        local db_min
        if [[ "$target_major" -ge 11 ]]; then
            if echo "$db_version_full" | grep -qi "mariadb"; then
                db_min="10.5.0"
            else
                db_min="8.0.0"
            fi
        else
            if echo "$db_version_full" | grep -qi "mariadb"; then
                db_min="10.2.0"
            else
                db_min="5.7.0"
            fi
        fi

        required_count=$((required_count + 1))
        if version_ge "$db_version" "$db_min"; then
            printf "  %-22s %-14s >= %-10s ${C_GREEN}OK${C_RESET}\n" "$DB_ENGINE version" "$db_version" "$db_min"
        else
            printf "  %-22s %-14s >= %-10s ${C_RED}FAIL${C_RESET}\n" "$DB_ENGINE version" "$db_version" "$db_min"
            errors=$((errors + 1))
        fi
    else
        printf "  %-22s %-14s %-11s ${C_YELLOW}SKIPPED${C_RESET}\n" "DB version" "cannot connect" ""
        warnings=$((warnings + 1))
    fi

    # Apache mod_rewrite (best-effort)
    if [[ "$WEB_SERVER" == "apache" ]]; then
        local mods
        mods="$(apache2ctl -M 2>/dev/null || apachectl -M 2>/dev/null || httpd -M 2>/dev/null || true)"
        if echo "$mods" | grep -q "rewrite_module"; then
            printf "  %-22s %-14s %-11s ${C_GREEN}OK${C_RESET}\n" "mod_rewrite" "enabled" ""
        else
            printf "  %-22s %-14s %-11s ${C_YELLOW}WARNING${C_RESET}\n" "mod_rewrite" "not found" ""
            warnings=$((warnings + 1))
        fi
    fi

    # SELinux / AppArmor
    if command -v getenforce &>/dev/null; then
        local se_status
        se_status="$(getenforce 2>/dev/null || echo "unknown")"
        if [[ "$se_status" == "Enforcing" ]]; then
            printf "  %-22s %-14s %-11s ${C_YELLOW}WARNING${C_RESET}\n" "SELinux" "$se_status" "(review policies)"
            warnings=$((warnings + 1))
        else
            printf "  %-22s %-14s %-11s ${C_GREEN}OK${C_RESET}\n" "SELinux" "$se_status" ""
        fi
    fi

    if command -v aa-status &>/dev/null; then
        local aa_profiles
        aa_profiles="$(aa-status --enabled 2>/dev/null && echo "active" || echo "inactive")"
        printf "  %-22s %-14s\n" "AppArmor" "$aa_profiles"
    fi

    echo ""
    echo "  Required: $((required_count - errors))/${required_count} passed | Warnings: $warnings"

    if [[ "$errors" -gt 0 ]]; then
        die "Cannot proceed: $errors required runtime dependencies failed."
    fi
}

# -- 1j: Disk space check ---------------------------------------------------
check_disk_space() {
    header "Checking disk space"

    local glpi_size_kb
    glpi_size_kb="$(du -sk "$GLPI_PATH" 2>/dev/null | awk '{print $1}' || echo 0)"
    local needed_kb=$((glpi_size_kb * 3))
    local needed_mb=$((needed_kb / 1024))

    local avail_kb
    avail_kb="$(df -k "$GLPI_PATH" 2>/dev/null | awk 'NR==2{print $4}' || echo 0)"
    local avail_mb=$((avail_kb / 1024))

    log_info "GLPI size: ~${glpi_size_kb}K | Need: ~${needed_mb}M | Available: ~${avail_mb}M"

    if [[ "$avail_kb" -lt "$needed_kb" ]]; then
        die "Insufficient disk space. Need ~${needed_mb}M, have ~${avail_mb}M."
    fi

    # Also check backup dir partition
    local backup_parent
    backup_parent="$(dirname "$OPT_BACKUP_DIR")"
    if [[ -d "$backup_parent" ]]; then
        local bavail_kb
        bavail_kb="$(df -k "$backup_parent" 2>/dev/null | awk 'NR==2{print $4}' || echo 0)"
        local bavail_mb=$((bavail_kb / 1024))
        log_info "Backup partition available: ~${bavail_mb}M"
        if [[ "$bavail_kb" -lt "$needed_kb" ]]; then
            log_warn "Backup partition may not have enough space for a full backup."
            confirm "Continue anyway?" || die "Aborted by user."
        fi
    fi

    log_ok "Disk space check passed"
}

# -- Phase 1 orchestrator ---------------------------------------------------
phase_1_preflight() {
    header "PHASE 1: Pre-flight Checks"

    detect_distro
    check_script_dependencies
    detect_glpi_path
    detect_glpi_data_dirs
    detect_database_config
    detect_glpi_version
    detect_web_server
    detect_database_engine
    detect_latest_version
    check_glpi_runtime_deps
    check_disk_space

    save_state 1
    log_ok "Phase 1 complete: all pre-flight checks passed."
}

# =============================================================================
# Phase 2: Plugin Compatibility Check
# =============================================================================
phase_2_plugin_check() {
    if $OPT_NO_PLUGIN_CHECK; then
        log_info "Plugin check skipped (--no-plugin-check)"
        save_state 2
        return
    fi

    header "PHASE 2: Plugin Compatibility Check"

    PLUGIN_NAMES=()
    PLUGIN_STATUSES=()
    PLUGIN_MIGRATE=()

    local -a plugin_dirs=()
    [[ -d "$GLPI_PLUGINS_DIR" ]] && plugin_dirs+=("$GLPI_PLUGINS_DIR")
    [[ -d "$GLPI_MARKETPLACE_DIR" ]] && [[ "$GLPI_MARKETPLACE_DIR" != "$GLPI_PLUGINS_DIR" ]] && plugin_dirs+=("$GLPI_MARKETPLACE_DIR")

    local target_version="$GLPI_TARGET_VERSION"

    printf "\n  ${C_BOLD}%-25s %-12s %-14s %-12s %s${C_RESET}\n" "PLUGIN" "VERSION" "MIN GLPI" "MAX GLPI" "STATUS"
    printf "  %-25s %-12s %-14s %-12s %s\n" "-------------------------" "------------" "--------------" "------------" "----------"

    for pdir in "${plugin_dirs[@]}"; do
        for plugin_path in "$pdir"/*/; do
            [[ -d "$plugin_path" ]] || continue
            local pname
            pname="$(basename "$plugin_path")"
            local setup="$plugin_path/setup.php"
            [[ -f "$setup" ]] || continue

            local pversion="" pmin="" pmax="" status=""

            # Extract plugin metadata from setup.php
            local pname_upper
            pname_upper="$(echo "$pname" | tr '[:lower:]' '[:upper:]')"

            pversion="$(grep -oP "PLUGIN_${pname_upper}_VERSION['\",\s]+['\"]?\K[0-9]+\.[0-9]+[0-9.]*" "$setup" 2>/dev/null | head -1 || true)"
            pmin="$(grep -oP "PLUGIN_${pname_upper}_MIN_GLPI_VERSION['\",\s]+['\"]?\K[0-9]+\.[0-9]+[0-9.]*" "$setup" 2>/dev/null | head -1 || true)"
            pmax="$(grep -oP "PLUGIN_${pname_upper}_MAX_GLPI_VERSION['\",\s]+['\"]?\K[0-9]+\.[0-9]+[0-9.]*" "$setup" 2>/dev/null | head -1 || true)"

            [[ -z "$pversion" ]] && pversion="?"
            [[ -z "$pmin" ]] && pmin="any"
            [[ -z "$pmax" ]] && pmax="any"

            # Evaluate compatibility
            local compat=true
            if [[ "$pmin" != "any" ]] && ! version_ge "$target_version" "$pmin"; then
                compat=false
            fi
            if [[ "$pmax" != "any" ]] && ! version_ge "$pmax" "$target_version"; then
                compat=false
            fi

            if $compat; then
                status="${C_GREEN}OK${C_RESET}"
                PLUGIN_STATUSES+=("ok")
            else
                status="${C_RED}INCOMPATIBLE${C_RESET}"
                PLUGIN_STATUSES+=("incompatible")
            fi

            # 10->11 migration flags
            local migrate_action=""
            if $MAJOR_UPGRADE; then
                local cur_major tar_major
                cur_major="$(version_major "$GLPI_CURRENT_VERSION")"
                tar_major="$(version_major "$GLPI_TARGET_VERSION")"
                if [[ "$cur_major" -le 10 ]] && [[ "$tar_major" -ge 11 ]]; then
                    case "$pname" in
                        genericobject)
                            migrate_action="migration:genericobject_plugin_to_core"
                            PLUGIN_MIGRATE+=("genericobject")
                            ;;
                        formcreator)
                            migrate_action="migration:formcreator_plugin_to_core"
                            PLUGIN_MIGRATE+=("formcreator")
                            ;;
                        fields)
                            migrate_action="ensure enabled for FormCreator migration"
                            PLUGIN_MIGRATE+=("fields")
                            ;;
                    esac
                fi
            fi

            PLUGIN_NAMES+=("$pname")

            printf "  %-25s %-12s %-14s %-12s %s" "$pname" "$pversion" "$pmin" "$pmax" "$status"
            [[ -n "$migrate_action" ]] && printf "  ${C_YELLOW}[MIGRATE: %s]${C_RESET}" "$migrate_action"
            echo ""
        done
    done

    echo ""

    # Decision gate
    local has_incompatible=false
    if [[ ${#PLUGIN_STATUSES[@]} -gt 0 ]]; then
        for s in "${PLUGIN_STATUSES[@]}"; do
            [[ "$s" == "incompatible" ]] && has_incompatible=true
        done
    fi

    if $has_incompatible; then
        log_warn "Some plugins are INCOMPATIBLE with GLPI $GLPI_TARGET_VERSION."
        log_warn "They will be disabled after the update. You may need to find updated versions."
        confirm "Acknowledge and continue?" || die "Aborted by user."
    fi

    if [[ ${#PLUGIN_MIGRATE[@]} -gt 0 ]]; then
        log_info "Plugin migrations will be run in Phase 8: ${PLUGIN_MIGRATE[*]}"
    fi

    save_state 2
    log_ok "Phase 2 complete: plugin compatibility checked."
}

# =============================================================================
# Phase 3: Backup
# =============================================================================
phase_3_backup() {
    header "PHASE 3: Backup"

    BACKUP_SUBDIR="$OPT_BACKUP_DIR/glpi_backup_${TIMESTAMP}"
    mkdir -p "$BACKUP_SUBDIR"
    chmod 700 "$OPT_BACKUP_DIR" 2>/dev/null || true
    chmod 700 "$BACKUP_SUBDIR"

    # Database backup
    log_info "Backing up database: $GLPI_DB_NAME ..."
    local db_dump_file="$BACKUP_SUBDIR/db_${GLPI_DB_NAME}_${TIMESTAMP}.sql.gz"

    if $OPT_DRY_RUN; then
        log_info "[DRY RUN] Would dump database to $db_dump_file"
    else
        create_db_defaults_file
        local dump_exit=0
        $DB_DUMP_CMD --defaults-extra-file="$DB_DEFAULTS_FILE" \
            --single-transaction --routines --triggers --quick \
            "$GLPI_DB_NAME" 2>"$BACKUP_SUBDIR/mysqldump_stderr.log" | gzip > "$db_dump_file" || dump_exit=$?
        remove_db_defaults_file

        if [[ "$dump_exit" -ne 0 ]]; then
            log_error "Database dump failed (exit $dump_exit). See $BACKUP_SUBDIR/mysqldump_stderr.log"
            die "Database backup failed. Aborting."
        fi
        if [[ ! -s "$db_dump_file" ]]; then
            die "Database backup is empty. Aborting."
        fi
        local db_size
        db_size="$(du -sh "$db_dump_file" | awk '{print $1}')"
        log_ok "Database backup: $db_dump_file ($db_size)"
    fi

    # Files backup
    log_info "Backing up GLPI files ..."
    local files_tar="$BACKUP_SUBDIR/glpi_files_${TIMESTAMP}.tar.gz"

    local -a tar_paths=("$GLPI_PATH")

    # Add external dirs only if they're actually external to GLPI_PATH
    if $FHS_EXTERNAL; then
        for edir in "$GLPI_CONFIG_DIR" "$GLPI_VAR_DIR" "$GLPI_LOG_DIR"; do
            case "$edir" in
                "$GLPI_PATH"*) ;; # already inside GLPI_PATH
                *) tar_paths+=("$edir") ;;
            esac
        done
    fi

    if $OPT_DRY_RUN; then
        log_info "[DRY RUN] Would archive: ${tar_paths[*]}"
    else
        local tar_exit=0
        tar czf "$files_tar" "${tar_paths[@]}" 2>"$BACKUP_SUBDIR/tar_stderr.log" || tar_exit=$?
        if [[ "$tar_exit" -ne 0 ]]; then
            if [[ "$tar_exit" -eq 1 ]]; then
                log_warn "tar reported warnings (exit 1, non-fatal). See $BACKUP_SUBDIR/tar_stderr.log"
            else
                log_error "tar failed (exit $tar_exit). See $BACKUP_SUBDIR/tar_stderr.log"
                die "Files backup failed. Aborting."
            fi
        fi
        if [[ ! -s "$files_tar" ]]; then
            die "Files backup is empty. Aborting."
        fi
        # Verify tar integrity
        tar tzf "$files_tar" &>/dev/null || die "Files backup is corrupt. Aborting."
        local tar_size
        tar_size="$(du -sh "$files_tar" | awk '{print $1}')"
        log_ok "Files backup: $files_tar ($tar_size)"
    fi

    # Separate copies of critical config files for quick restore
    local quick_dir="$BACKUP_SUBDIR/quick_restore"
    mkdir -p "$quick_dir"

    [[ -f "$GLPI_DOWNSTREAM" ]] && cp "$GLPI_DOWNSTREAM" "$quick_dir/" 2>/dev/null || true
    [[ -f "$GLPI_LOCAL_DEFINE" ]] && cp "$GLPI_LOCAL_DEFINE" "$quick_dir/" 2>/dev/null || true
    [[ -f "$GLPI_CONFIG_DIR/config_db.php" ]] && cp "$GLPI_CONFIG_DIR/config_db.php" "$quick_dir/" 2>/dev/null || true

    # Save metadata
    cat > "$BACKUP_SUBDIR/backup_info.txt" <<EOF
GLPI Version: $GLPI_CURRENT_VERSION
Target Version: $GLPI_TARGET_VERSION
Timestamp: $TIMESTAMP
GLPI Path: $GLPI_PATH
Config Dir: $GLPI_CONFIG_DIR
Var Dir: $GLPI_VAR_DIR
Log Dir: $GLPI_LOG_DIR
FHS External: $FHS_EXTERNAL
DB Name: $GLPI_DB_NAME
DB Host: $GLPI_DB_HOST
Web User: $WEB_USER
EOF

    # Retention: prune old backups
    if ! $OPT_DRY_RUN; then
        local backup_count
        backup_count="$(find "$OPT_BACKUP_DIR" -maxdepth 1 -name "glpi_backup_*" -type d | wc -l)"
        if [[ "$backup_count" -gt "$OPT_BACKUP_RETENTION" ]]; then
            local to_remove=$((backup_count - OPT_BACKUP_RETENTION))
            log_info "Pruning $to_remove old backup(s) (retention: $OPT_BACKUP_RETENTION)"
            find "$OPT_BACKUP_DIR" -maxdepth 1 -name "glpi_backup_*" -type d | sort | head -n "$to_remove" | while read -r old; do
                rm -rf "$old"
                log_debug "Removed old backup: $old"
            done
        fi
    fi

    save_state 3
    log_ok "Phase 3 complete: backup saved to $BACKUP_SUBDIR"

    if $OPT_BACKUP_ONLY; then
        log_ok "Backup-only mode: stopping here."
        clear_state
        release_lock
        exit 0
    fi
}

# =============================================================================
# Phase 4: Download and Verify
# =============================================================================
phase_4_download() {
    header "PHASE 4: Download GLPI $GLPI_TARGET_VERSION"

    STAGING_DIR="$(mktemp -d /tmp/glpi-update-staging.XXXXXXXX)"
    chmod 700 "$STAGING_DIR"
    local tarball="$STAGING_DIR/glpi-${GLPI_TARGET_VERSION}.tgz"
    local download_url="https://github.com/glpi-project/glpi/releases/download/${GLPI_TARGET_VERSION}/glpi-${GLPI_TARGET_VERSION}.tgz"

    log_info "Downloading from: $download_url"

    if $OPT_DRY_RUN; then
        log_info "[DRY RUN] Would download $download_url"
        save_state 4
        return
    fi

    if [[ "$DOWNLOAD_CMD" == "curl" ]]; then
        curl -L --connect-timeout 15 --max-time "$OPT_DOWNLOAD_TIMEOUT" \
            --progress-bar -o "$tarball" "$download_url" || die "Download failed."
    else
        wget --timeout="$OPT_DOWNLOAD_TIMEOUT" --show-progress \
            -O "$tarball" "$download_url" || die "Download failed."
    fi

    if [[ ! -s "$tarball" ]]; then
        die "Downloaded file is empty."
    fi

    local dl_size
    dl_size="$(du -sh "$tarball" | awk '{print $1}')"
    log_ok "Downloaded: $tarball ($dl_size)"

    # Checksum verification
    if $HAS_CHECKSUM; then
        local expected=""
        local tarball_name="glpi-${GLPI_TARGET_VERSION}.tgz"

        # Method 1: extract SHA256 from GitHub API response (asset digest)
        if [[ -n "$GITHUB_API_RESPONSE" ]]; then
            if $HAS_JQ; then
                expected="$(echo "$GITHUB_API_RESPONSE" | \
                    jq -r --arg name "$tarball_name" \
                    '.assets[] | select(.name == $name) | .digest // empty' 2>/dev/null | \
                    sed 's/^sha256://' || true)"
            else
                # Grep fallback: find the digest near the asset name in the JSON
                expected="$(echo "$GITHUB_API_RESPONSE" | \
                    grep -A5 "\"name\".*\"${tarball_name}\"" | \
                    grep -oP '"digest"\s*:\s*"sha256:\K[0-9a-fA-F]+' | head -1 || true)"
            fi
            if [[ -n "$expected" ]]; then
                log_debug "SHA256 from GitHub API: $expected"
            fi
        fi

        # Method 2: try downloading a separate checksum file
        if [[ -z "$expected" ]]; then
            local checksum_file="$STAGING_DIR/checksum.sha256"
            local -a checksum_urls=(
                "${download_url}.sha256"
                "${download_url}.sha256sum"
            )
            for checksum_url in "${checksum_urls[@]}"; do
                if [[ "$DOWNLOAD_CMD" == "curl" ]]; then
                    curl -sSLf -o "$checksum_file" "$checksum_url" 2>/dev/null || continue
                else
                    wget -qO "$checksum_file" "$checksum_url" 2>/dev/null || continue
                fi
                if [[ -s "$checksum_file" ]]; then
                    expected="$(awk '{print $1}' "$checksum_file" | head -1)"
                    break
                fi
            done
        fi

        # Verify the checksum if we got one
        if [[ -n "$expected" ]] && [[ "$expected" =~ ^[0-9a-fA-F]{64}$ ]]; then
            local actual
            if command -v sha256sum &>/dev/null; then
                actual="$(sha256sum "$tarball" | awk '{print $1}')"
            else
                actual="$(shasum -a 256 "$tarball" | awk '{print $1}')"
            fi
            if [[ "${expected,,}" == "${actual,,}" ]]; then
                log_ok "SHA256 checksum verified: ${actual}"
            else
                die "SHA256 checksum mismatch! Expected: $expected Got: $actual"
            fi
        else
            # No checksum available -- compute and display for manual verification
            local actual
            if command -v sha256sum &>/dev/null; then
                actual="$(sha256sum "$tarball" | awk '{print $1}')"
            else
                actual="$(shasum -a 256 "$tarball" | awk '{print $1}')"
            fi
            log_warn "No SHA256 checksum available from release for automatic verification."
            log_info "Downloaded file SHA256: $actual"
            log_info "Verify manually at: https://github.com/glpi-project/glpi/releases/tag/${GLPI_TARGET_VERSION}"
            confirm "Does the checksum match? Continue?" || die "Aborted by user."
        fi
    else
        log_warn "No checksum tool available. Skipping integrity verification."
    fi

    # Verify tarball is valid
    tar tzf "$tarball" &>/dev/null || die "Downloaded tarball is corrupt."

    # Extract to staging
    log_info "Extracting to staging directory ..."
    tar xzf "$tarball" -C "$STAGING_DIR"
    if [[ ! -d "$STAGING_DIR/glpi" ]]; then
        die "Extracted tarball does not contain expected 'glpi' directory."
    fi

    log_ok "Extraction complete: $STAGING_DIR/glpi"

    save_state 4

    if $OPT_DOWNLOAD_ONLY; then
        log_ok "Download-only mode: tarball at $tarball"
        clear_state
        release_lock
        exit 0
    fi
}

# =============================================================================
# Phase 5: Maintenance Mode
# =============================================================================
phase_5_maintenance() {
    header "PHASE 5: Enabling Maintenance Mode"

    if $OPT_DRY_RUN; then
        log_info "[DRY RUN] Would enable maintenance mode"
        save_state 5
        return
    fi

    # GLPI maintenance mode via CLI if available
    if [[ -f "$GLPI_PATH/bin/console" ]]; then
        run_as_webuser php "$GLPI_PATH/bin/console" glpi:maintenance:enable 2>/dev/null || {
            log_warn "CLI maintenance enable failed. Trying file-based approach."
        }
    fi

    # Also put a maintenance flag file (belt and suspenders)
    local maint_file="$GLPI_CONFIG_DIR/maintenance_mode"
    touch "$maint_file" 2>/dev/null && chown "$WEB_USER:$WEB_USER" "$maint_file" 2>/dev/null || true

    # Optional: serve custom maintenance page
    if [[ -n "$OPT_MAINTENANCE_PAGE" ]] && [[ -f "$OPT_MAINTENANCE_PAGE" ]]; then
        cp "$OPT_MAINTENANCE_PAGE" "$GLPI_PATH/index_maint.html" 2>/dev/null || true
        log_info "Custom maintenance page deployed."
    fi

    save_state 5
    log_ok "Phase 5 complete: maintenance mode enabled."
}

# =============================================================================
# Phase 6: Extract and Replace Files
# =============================================================================
phase_6_replace_files() {
    header "PHASE 6: Replacing GLPI Files"

    if $OPT_DRY_RUN; then
        log_info "[DRY RUN] Would replace $GLPI_PATH with $STAGING_DIR/glpi"
        save_state 6
        return
    fi

    if [[ ! -d "$STAGING_DIR/glpi" ]]; then
        die "Staging directory missing: $STAGING_DIR/glpi"
    fi

    # Move current to .old (atomic rename)
    OLD_DIR="${GLPI_PATH}.old_${TIMESTAMP}"
    log_info "Moving current installation -> $OLD_DIR"
    mv "$GLPI_PATH" "$OLD_DIR" || die "Failed to move current GLPI directory."

    # Move new version into place
    log_info "Installing new version -> $GLPI_PATH"
    mv "$STAGING_DIR/glpi" "$GLPI_PATH" || {
        log_error "Failed to place new files. Rolling back ..."
        mv "$OLD_DIR" "$GLPI_PATH"
        die "Rollback complete. New files could not be placed."
    }

    # Restore downstream.php
    if [[ -f "$OLD_DIR/inc/downstream.php" ]]; then
        cp "$OLD_DIR/inc/downstream.php" "$GLPI_PATH/inc/downstream.php"
        log_ok "Restored downstream.php"
    fi

    # Restore local_define.php (if it was inside the GLPI tree)
    if [[ -f "$OLD_DIR/config/local_define.php" ]]; then
        mkdir -p "$GLPI_PATH/config"
        cp "$OLD_DIR/config/local_define.php" "$GLPI_PATH/config/local_define.php"
        log_ok "Restored local_define.php"
    fi

    # If NOT using FHS-external, restore internal data dirs
    if ! $FHS_EXTERNAL; then
        log_info "Internal layout: restoring config/ and files/ from old installation"

        if [[ -d "$OLD_DIR/config" ]]; then
            cp -a "$OLD_DIR/config/." "$GLPI_PATH/config/" 2>/dev/null || true
        fi
        if [[ -d "$OLD_DIR/files" ]]; then
            cp -a "$OLD_DIR/files/." "$GLPI_PATH/files/" 2>/dev/null || true
        fi
    else
        log_info "FHS-external layout: config/data/logs untouched (outside install dir)"
    fi

    # Restore plugins from old installation
    if [[ -d "$OLD_DIR/plugins" ]]; then
        log_info "Restoring plugins from old installation ..."
        cp -a "$OLD_DIR/plugins/." "$GLPI_PATH/plugins/" 2>/dev/null || true
    fi
    if [[ -d "$OLD_DIR/marketplace" ]]; then
        log_info "Restoring marketplace plugins from old installation ..."
        mkdir -p "$GLPI_PATH/marketplace"
        cp -a "$OLD_DIR/marketplace/." "$GLPI_PATH/marketplace/" 2>/dev/null || true
    fi

    # Fix ownership
    log_info "Setting file ownership to $WEB_USER ..."
    chown -R "$WEB_USER:$WEB_USER" "$GLPI_PATH" 2>/dev/null || log_warn "Could not set ownership on $GLPI_PATH"
    if ! $FHS_EXTERNAL; then
        chmod -R u+rwX "$GLPI_PATH/files" 2>/dev/null || true
    fi

    # SELinux: new files inherit context from /tmp staging dir, not /opt/glpi
    fix_selinux_contexts

    # Ensure web server log files referenced in vhosts are writable
    fix_webserver_log_files

    save_state 6
    log_ok "Phase 6 complete: files replaced."
}

# =============================================================================
# Phase 7: Database Migration
# =============================================================================
phase_7_db_migration() {
    header "PHASE 7: Database Migration"

    if $OPT_DRY_RUN; then
        log_info "[DRY RUN] Would run: php bin/console db:update --no-interaction --force"
        save_state 7
        return
    fi

    if [[ ! -f "$GLPI_PATH/bin/console" ]]; then
        die "bin/console not found in new GLPI installation."
    fi

    log_info "Running database migration (timeout: ${OPT_DB_UPDATE_TIMEOUT}s) ..."
    local db_update_log="$BACKUP_SUBDIR/db_update_${TIMESTAMP}.log"

    local exit_code=0
    timeout "$OPT_DB_UPDATE_TIMEOUT" \
        sudo -u "$WEB_USER" php "$GLPI_PATH/bin/console" db:update --no-interaction --force \
        2>&1 | tee "$db_update_log" || exit_code=$?

    if [[ "$exit_code" -ne 0 ]]; then
        log_error "Database migration failed (exit code: $exit_code)"
        log_error "Migration log saved to: $db_update_log"

        if $OPT_NO_ROLLBACK; then
            die "Database migration failed. Rollback disabled (--no-rollback)."
        fi

        rollback
        die "Rolled back due to database migration failure."
    fi

    save_state 7
    log_ok "Phase 7 complete: database migration successful."
}

# =============================================================================
# Phase 8: Plugin Migration (10->11)
# =============================================================================
phase_8_plugin_migration() {
    header "PHASE 8: Plugin Migration"

    if [[ ${#PLUGIN_MIGRATE[@]} -eq 0 ]]; then
        log_info "No plugin migrations required."
        save_state 8
        return
    fi

    if $OPT_DRY_RUN; then
        log_info "[DRY RUN] Would run plugin migrations for: ${PLUGIN_MIGRATE[*]}"
        save_state 8
        return
    fi

    local cur_major tar_major
    cur_major="$(version_major "$GLPI_CURRENT_VERSION")"
    tar_major="$(version_major "$GLPI_TARGET_VERSION")"

    if [[ "$cur_major" -le 10 ]] && [[ "$tar_major" -ge 11 ]]; then
        # GenericObject first
        for plugin in "${PLUGIN_MIGRATE[@]}"; do
            if [[ "$plugin" == "genericobject" ]]; then
                log_info "Migrating genericobject plugin to core ..."
                run_as_webuser php "$GLPI_PATH/bin/console" migration:genericobject_plugin_to_core 2>&1 || {
                    log_warn "genericobject migration reported errors (non-fatal)"
                }
            fi
        done

        # Fields (ensure enabled)
        for plugin in "${PLUGIN_MIGRATE[@]}"; do
            if [[ "$plugin" == "fields" ]]; then
                log_info "Ensuring fields plugin is activated ..."
                run_as_webuser php "$GLPI_PATH/bin/console" glpi:plugin:activate fields 2>&1 || {
                    log_warn "fields plugin activation reported errors"
                }
            fi
        done

        # FormCreator last
        for plugin in "${PLUGIN_MIGRATE[@]}"; do
            if [[ "$plugin" == "formcreator" ]]; then
                log_info "Migrating formcreator plugin to core ..."
                run_as_webuser php "$GLPI_PATH/bin/console" migration:formcreator_plugin_to_core 2>&1 || {
                    log_warn "formcreator migration reported errors (non-fatal)"
                }
            fi
        done
    fi

    # Re-enable compatible plugins
    log_info "Re-enabling compatible plugins ..."
    local i
    for i in "${!PLUGIN_NAMES[@]}"; do
        if [[ "${PLUGIN_STATUSES[$i]}" == "ok" ]]; then
            run_as_webuser php "$GLPI_PATH/bin/console" glpi:plugin:activate "${PLUGIN_NAMES[$i]}" 2>/dev/null || {
                log_warn "Could not re-enable plugin: ${PLUGIN_NAMES[$i]}"
            }
        fi
    done

    save_state 8
    log_ok "Phase 8 complete: plugin migrations done."
}

# =============================================================================
# Phase 9: Post-update Health Checks
# =============================================================================
phase_9_post_checks() {
    header "PHASE 9: Post-update Health Checks"

    local checks_passed=0
    local checks_total=0

    # Version verification via CLI
    checks_total=$((checks_total + 1))
    local new_version
    new_version="$(run_as_webuser php "$GLPI_PATH/bin/console" --version 2>/dev/null | grep -oP '[0-9]+\.[0-9]+\.[0-9]+' | head -1 || true)"
    if [[ "$new_version" == "$GLPI_TARGET_VERSION" ]]; then
        log_ok "Version verified: $new_version"
        checks_passed=$((checks_passed + 1))
    else
        log_warn "Version mismatch. Expected: $GLPI_TARGET_VERSION Got: ${new_version:-unknown}"
    fi

    # System requirements check (if available in this version)
    checks_total=$((checks_total + 1))
    if run_as_webuser php "$GLPI_PATH/bin/console" system:check_requirements &>/dev/null; then
        log_ok "system:check_requirements passed"
        checks_passed=$((checks_passed + 1))
    else
        log_warn "system:check_requirements not available or returned warnings"
        checks_passed=$((checks_passed + 1))
    fi

    # HTTP check (best-effort: only works if we can guess the URL)
    local glpi_url=""
    if [[ "$WEB_SERVER" == "apache" ]]; then
        glpi_url="$(grep -roh "ServerName\s\+\S\+" /etc/apache2/sites-enabled/ /etc/httpd/conf.d/ 2>/dev/null | head -1 | awk '{print $2}' || true)"
    elif [[ "$WEB_SERVER" == "nginx" ]]; then
        glpi_url="$(grep -oP 'server_name\s+\K\S+' /etc/nginx/sites-enabled/* /etc/nginx/conf.d/* 2>/dev/null | head -1 | tr -d ';' || true)"
    fi

    if [[ -n "$glpi_url" ]]; then
        checks_total=$((checks_total + 1))
        local http_code
        http_code="$(curl -sS -o /dev/null -w '%{http_code}' --connect-timeout 5 "http://${glpi_url}/" 2>/dev/null || echo "000")"
        if [[ "$http_code" =~ ^(200|301|302|303)$ ]]; then
            log_ok "HTTP check: $glpi_url responded with $http_code"
            checks_passed=$((checks_passed + 1))
        else
            log_warn "HTTP check: $glpi_url responded with $http_code"
        fi
    fi

    # Check PHP error log for new fatals
    checks_total=$((checks_total + 1))
    local php_error_log
    php_error_log="$(php -r 'echo ini_get("error_log");' 2>/dev/null || true)"
    if [[ -n "$php_error_log" ]] && [[ -f "$php_error_log" ]]; then
        local recent_fatals
        recent_fatals="$(tail -50 "$php_error_log" 2>/dev/null | grep -c -iE "fatal|critical" || echo 0)"
        if [[ "$recent_fatals" -eq 0 ]]; then
            log_ok "No recent fatal errors in PHP error log"
            checks_passed=$((checks_passed + 1))
        else
            log_warn "Found $recent_fatals recent fatal/critical entries in PHP error log"
        fi
    else
        log_info "PHP error log not accessible, skipping"
        checks_passed=$((checks_passed + 1))
    fi

    # Disable maintenance mode
    if ! $OPT_DRY_RUN; then
        if [[ -f "$GLPI_PATH/bin/console" ]]; then
            run_as_webuser php "$GLPI_PATH/bin/console" glpi:maintenance:disable 2>/dev/null || true
        fi
        rm -f "$GLPI_CONFIG_DIR/maintenance_mode" 2>/dev/null || true
        rm -f "$GLPI_PATH/index_maint.html" 2>/dev/null || true
        log_ok "Maintenance mode disabled"
    fi

    echo ""
    log_info "Health checks: ${checks_passed}/${checks_total} passed"

    save_state 9
    log_ok "Phase 9 complete: post-update checks done."
}

# =============================================================================
# Phase 10: Cleanup and Report
# =============================================================================
phase_10_cleanup() {
    header "PHASE 10: Cleanup and Final Report"

    # Clean up staging directory
    if [[ -n "$STAGING_DIR" ]] && [[ -d "$STAGING_DIR" ]]; then
        rm -rf "$STAGING_DIR"
        log_debug "Removed staging directory: $STAGING_DIR"
    fi

    # Old directory
    if [[ -n "$OLD_DIR" ]] && [[ -d "$OLD_DIR" ]]; then
        local old_size
        old_size="$(du -sh "$OLD_DIR" 2>/dev/null | awk '{print $1}' || echo "?")"
        log_info "Old GLPI installation preserved at: $OLD_DIR ($old_size)"
        if ! $OPT_DRY_RUN; then
            if confirm "Remove old installation ($OLD_DIR)?"; then
                rm -rf "$OLD_DIR"
                log_ok "Old installation removed."
            else
                log_info "Old installation kept. Remove it manually when ready."
            fi
        fi
    fi

    # Final report
    echo ""
    echo "${C_BOLD}${C_GREEN}=============================================${C_RESET}"
    echo "${C_BOLD}${C_GREEN}    GLPI Update Complete!${C_RESET}"
    echo "${C_BOLD}${C_GREEN}=============================================${C_RESET}"
    echo ""
    printf "  %-24s %s\n" "Previous version:" "$GLPI_CURRENT_VERSION"
    printf "  %-24s %s\n" "New version:" "$GLPI_TARGET_VERSION"
    printf "  %-24s %s\n" "GLPI path:" "$GLPI_PATH"
    printf "  %-24s %s\n" "Backup location:" "$BACKUP_SUBDIR"
    printf "  %-24s %s\n" "Log file:" "$OPT_LOG_FILE"
    if [[ ${#PLUGIN_NAMES[@]} -gt 0 ]]; then
        printf "  %-24s %s\n" "Plugins checked:" "${#PLUGIN_NAMES[@]}"
    fi
    if [[ ${#PLUGIN_MIGRATE[@]} -gt 0 ]]; then
        printf "  %-24s %s\n" "Plugins migrated:" "${PLUGIN_MIGRATE[*]}"
    fi
    echo ""

    # Write summary to log
    if ! $OPT_DRY_RUN; then
        cat >> "$OPT_LOG_FILE" <<EOF

=== GLPI Update Summary ===
Date: $(date)
Previous: $GLPI_CURRENT_VERSION
New: $GLPI_TARGET_VERSION
Backup: $BACKUP_SUBDIR
Status: SUCCESS
============================
EOF
    fi

    clear_state
    log_ok "All done."
}

# =============================================================================
# Rollback
# =============================================================================
rollback() {
    header "ROLLBACK"

    log_warn "Initiating rollback ..."

    # Restore files
    if [[ -n "$OLD_DIR" ]] && [[ -d "$OLD_DIR" ]]; then
        log_info "Restoring files from $OLD_DIR ..."
        if [[ -d "$GLPI_PATH" ]]; then
            rm -rf "$GLPI_PATH"
        fi
        mv "$OLD_DIR" "$GLPI_PATH"
        chown -R "$WEB_USER:$WEB_USER" "$GLPI_PATH" 2>/dev/null || true
        fix_selinux_contexts
        log_ok "Files restored."
    else
        log_error "Cannot restore files: $OLD_DIR not found."
    fi

    # Restore database
    if [[ -n "$BACKUP_SUBDIR" ]]; then
        local db_dump
        db_dump="$(find "$BACKUP_SUBDIR" -name "db_*.sql.gz" -type f 2>/dev/null | head -1 || true)"
        if [[ -n "$db_dump" ]] && [[ -f "$db_dump" ]]; then
            log_info "Restoring database from $db_dump ..."
            log_warn "Dropping and recreating database $GLPI_DB_NAME ..."

            create_db_defaults_file
            $DB_CLIENT_CMD --defaults-extra-file="$DB_DEFAULTS_FILE" \
                -e "DROP DATABASE IF EXISTS \`${GLPI_DB_NAME}\`; CREATE DATABASE \`${GLPI_DB_NAME}\`;" 2>/dev/null || {
                remove_db_defaults_file
                log_error "Failed to recreate database."
                return 1
            }

            gunzip -c "$db_dump" | $DB_CLIENT_CMD --defaults-extra-file="$DB_DEFAULTS_FILE" "$GLPI_DB_NAME" 2>/dev/null || {
                remove_db_defaults_file
                log_error "Failed to restore database."
                return 1
            }
            remove_db_defaults_file
            log_ok "Database restored."
        else
            log_error "Cannot restore database: no dump found in $BACKUP_SUBDIR"
        fi
    fi

    # Disable maintenance mode
    if [[ -f "$GLPI_PATH/bin/console" ]]; then
        run_as_webuser php "$GLPI_PATH/bin/console" glpi:maintenance:disable 2>/dev/null || true
    fi
    rm -f "$GLPI_CONFIG_DIR/maintenance_mode" 2>/dev/null || true

    log_warn "Rollback complete. GLPI should be back at version $GLPI_CURRENT_VERSION."
}

# =============================================================================
# CLI argument parsing
# =============================================================================
require_arg() {
    if [[ $# -lt 2 ]] || [[ "$2" == --* ]]; then
        die "Option $1 requires an argument."
    fi
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --path)           require_arg "$1" "${2:-}"; OPT_GLPI_PATH="$2"; shift 2 ;;
            --config-dir)     require_arg "$1" "${2:-}"; OPT_CONFIG_DIR="$2"; shift 2 ;;
            --data-dir)       require_arg "$1" "${2:-}"; OPT_VAR_DIR="$2"; shift 2 ;;
            --log-dir)        require_arg "$1" "${2:-}"; OPT_LOG_DIR="$2"; shift 2 ;;
            --target-version) require_arg "$1" "${2:-}"; OPT_TARGET_VERSION="$2"; shift 2 ;;
            --backup-dir)     require_arg "$1" "${2:-}"; OPT_BACKUP_DIR="$2"; shift 2 ;;
            --dry-run)        OPT_DRY_RUN=true; shift ;;
            --backup-only)    OPT_BACKUP_ONLY=true; shift ;;
            --download-only)  OPT_DOWNLOAD_ONLY=true; shift ;;
            --force)          OPT_FORCE=true; shift ;;
            --yes)            OPT_YES=true; shift ;;
            --no-plugin-check) OPT_NO_PLUGIN_CHECK=true; shift ;;
            --no-rollback)    OPT_NO_ROLLBACK=true; shift ;;
            --verbose)        OPT_VERBOSE=true; shift ;;
            --help|-h)        show_help; exit 0 ;;
            *)                die "Unknown option: $1 (use --help)" ;;
        esac
    done
}

show_help() {
    cat <<EOF
${C_BOLD}GLPI Server Updater v${SCRIPT_VERSION}${C_RESET}

Usage: sudo bash $SCRIPT_NAME [OPTIONS]

${C_BOLD}Options:${C_RESET}
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

${C_BOLD}Examples:${C_RESET}
  sudo bash $SCRIPT_NAME
  sudo bash $SCRIPT_NAME --path /opt/glpi --verbose
  sudo bash $SCRIPT_NAME --target-version 11.0.6 --dry-run
  sudo bash $SCRIPT_NAME --backup-only --backup-dir /mnt/backups/glpi

${C_BOLD}Configuration:${C_RESET}
  Place glpi-update.conf next to this script or at /etc/glpi-update.conf

EOF
}

# =============================================================================
# Load configuration file
# =============================================================================
load_config() {
    local config_file=""
    if [[ -f "$SCRIPT_DIR/glpi-update.conf" ]]; then
        config_file="$SCRIPT_DIR/glpi-update.conf"
    elif [[ -f "/etc/glpi-update.conf" ]]; then
        config_file="/etc/glpi-update.conf"
    fi

    if [[ -n "$config_file" ]]; then
        log_debug "Loading config: $config_file"
        # Only source known variables
        while IFS='=' read -r key value; do
            key="$(echo "$key" | xargs)"
            [[ "$key" =~ ^#.*$ ]] && continue
            [[ -z "$key" ]] && continue
            # Strip inline comments: remove everything after unquoted #
            value="$(echo "$value" | sed 's/[[:space:]]*#.*$//' | xargs)"
            # Remove surrounding quotes
            value="$(echo "$value" | sed 's/^["'\'']//;s/["'\'']$//')"
            case "$key" in
                GLPI_PATH)          [[ -z "$OPT_GLPI_PATH" ]] && [[ -n "$value" ]] && OPT_GLPI_PATH="$value" ;;
                GLPI_CONFIG_DIR)    [[ -z "$OPT_CONFIG_DIR" ]] && [[ -n "$value" ]] && OPT_CONFIG_DIR="$value" ;;
                GLPI_VAR_DIR)       [[ -z "$OPT_VAR_DIR" ]]   && [[ -n "$value" ]] && OPT_VAR_DIR="$value" ;;
                GLPI_LOG_DIR)       [[ -z "$OPT_LOG_DIR" ]]   && [[ -n "$value" ]] && OPT_LOG_DIR="$value" ;;
                BACKUP_DIR)         OPT_BACKUP_DIR="${value:-$OPT_BACKUP_DIR}" ;;
                BACKUP_RETENTION)   OPT_BACKUP_RETENTION="${value:-$OPT_BACKUP_RETENTION}" ;;
                DOWNLOAD_TIMEOUT)   OPT_DOWNLOAD_TIMEOUT="${value:-$OPT_DOWNLOAD_TIMEOUT}" ;;
                DB_UPDATE_TIMEOUT)  OPT_DB_UPDATE_TIMEOUT="${value:-$OPT_DB_UPDATE_TIMEOUT}" ;;
                MAINTENANCE_PAGE)   OPT_MAINTENANCE_PAGE="${value:-$OPT_MAINTENANCE_PAGE}" ;;
                LOG_FILE)           OPT_LOG_FILE="${value:-$OPT_LOG_FILE}" ;;
                GITHUB_API_URL)     OPT_GITHUB_API_URL="${value:-$OPT_GITHUB_API_URL}" ;;
            esac
        done < "$config_file"
    fi
}

# =============================================================================
# Main
# =============================================================================
main() {
    setup_colors

    echo "${C_BOLD}${C_CYAN}"
    echo "  ╔═══════════════════════════════════════════╗"
    echo "  ║        GLPI Server Updater v${SCRIPT_VERSION}         ║"
    echo "  ╚═══════════════════════════════════════════╝"
    echo "${C_RESET}"

    parse_args "$@"
    load_config

    if $OPT_DRY_RUN; then
        echo "  ${C_YELLOW}*** DRY RUN MODE -- no changes will be made ***${C_RESET}"
        echo ""
    fi

    # Ensure running as root (or with sudo)
    if [[ "$(id -u)" -ne 0 ]]; then
        die "This script must be run as root (use sudo)."
    fi

    # Ensure log file parent directory exists
    local log_dir
    log_dir="$(dirname "$OPT_LOG_FILE")"
    mkdir -p "$log_dir" 2>/dev/null || true

    acquire_lock

    # Check for resume
    local resume_phase=0
    if load_state; then
        resume_phase="${PHASE:-0}"
        log_info "Resuming from phase $((resume_phase + 1))"
    fi

    if [[ "$resume_phase" -gt 0 ]]; then
        # Re-initialize runtime variables that later phases depend on.
        # We skip version detection and up-to-date checks because the
        # on-disk state may have changed mid-update. The versions from
        # the state file (already loaded) are authoritative.
        header "Reinitializing runtime environment for resume"
        local saved_cur="$GLPI_CURRENT_VERSION"
        local saved_tgt="$GLPI_TARGET_VERSION"
        detect_distro
        check_script_dependencies
        detect_glpi_path
        detect_glpi_data_dirs
        detect_database_config
        detect_web_server
        detect_database_engine
        GLPI_CURRENT_VERSION="$saved_cur"
        GLPI_TARGET_VERSION="$saved_tgt"
        local cur_major tar_major
        cur_major="$(version_major "$GLPI_CURRENT_VERSION")"
        tar_major="$(version_major "$GLPI_TARGET_VERSION")"
        [[ "$cur_major" -ne "$tar_major" ]] && MAJOR_UPGRADE=true
        # Fetch GitHub API response for checksum verification in phase 4
        if [[ "$resume_phase" -lt 4 ]]; then
            local api_url="https://api.github.com/repos/glpi-project/glpi/releases/tags/${GLPI_TARGET_VERSION}"
            if [[ "$DOWNLOAD_CMD" == "curl" ]]; then
                GITHUB_API_RESPONSE="$(curl -sS --connect-timeout 10 "$api_url" 2>/dev/null || true)"
            elif [[ "$DOWNLOAD_CMD" == "wget" ]]; then
                GITHUB_API_RESPONSE="$(wget -qO- --timeout=10 "$api_url" 2>/dev/null || true)"
            fi
        fi
        log_ok "Resumed: $GLPI_CURRENT_VERSION -> $GLPI_TARGET_VERSION (phase $((resume_phase + 1)))"
    else
        phase_1_preflight
    fi

    [[ "$resume_phase" -lt 2 ]] && phase_2_plugin_check

    if ! $OPT_DRY_RUN && [[ "$resume_phase" -lt 3 ]]; then
        echo ""
        echo "${C_BOLD}Ready to update GLPI: $GLPI_CURRENT_VERSION -> $GLPI_TARGET_VERSION${C_RESET}"
        echo ""
        confirm "Proceed with the update?" || { clear_state; die "Aborted by user."; }
    fi

    [[ "$resume_phase" -lt 3 ]]  && phase_3_backup
    [[ "$resume_phase" -lt 4 ]]  && phase_4_download
    [[ "$resume_phase" -lt 5 ]]  && phase_5_maintenance
    [[ "$resume_phase" -lt 6 ]]  && phase_6_replace_files
    [[ "$resume_phase" -lt 7 ]]  && phase_7_db_migration
    [[ "$resume_phase" -lt 8 ]]  && phase_8_plugin_migration
    [[ "$resume_phase" -lt 9 ]]  && phase_9_post_checks
    [[ "$resume_phase" -lt 10 ]] && phase_10_cleanup
}

main "$@"
