#!/usr/bin/env bash
set -e

# Available contracts in the workspace
CONTRACTS=("ip_registry" "atomic_swap" "zk_verifier")

# Build all contracts by default
TARGET="${1:-all}"

# Get the root directory of the project
ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
WASM_OUTPUT_DIR="${ROOT_DIR}/target/wasm32-unknown-unknown/release"

# Function to optimize a WASM contract artifact
optimize_contract() {
    local contract="$1"
    local wasm_file="${WASM_OUTPUT_DIR}/${contract}.wasm"
    
    if [ ! -f "$wasm_file" ]; then
        echo "Error: WASM artifact not found at $wasm_file"
        return 1
    fi
    
    echo "Optimizing ${contract}..."
    
    # Try to use stellar contract optimize (preferred method)
    if command -v stellar &> /dev/null; then
        if ! stellar contract optimize --wasm "$wasm_file"; then
            echo "Error: Failed to optimize ${contract} with stellar contract optimize"
            return 1
        fi
        echo "✓ ${contract} optimized successfully with stellar contract optimize"
    # Fall back to wasm-opt if available
    elif command -v wasm-opt &> /dev/null; then
        # wasm-opt modifies in-place with -O4 optimization level
        if ! wasm-opt -O4 -o "$wasm_file" "$wasm_file"; then
            echo "Error: Failed to optimize ${contract} with wasm-opt"
            return 1
        fi
        echo "✓ ${contract} optimized successfully with wasm-opt"
    else
        echo "Error: Neither 'stellar' nor 'wasm-opt' command found."
        echo "Please install either:"
        echo "  - Stellar CLI: https://github.com/stellar/stellar-cli"
        echo "  - wasm-opt: npm install -g wasm-opt or apt-get install wasm-opt"
        return 1
    fi
}

# Function to build a specific contract
build_contract() {
    local contract="$1"
    local path="contracts/${contract}"
    
    if [ ! -d "$path" ]; then
        echo "Error: Contract '${contract}' not found in workspace."
        exit 1
    fi
    
    echo "Building ${contract}..."
    cargo build --target wasm32-unknown-unknown --release -p "${contract}"
    echo "${contract} build complete."
    
    # Optimize the WASM artifact
    if ! optimize_contract "$contract"; then
        echo "Error: Optimization failed for ${contract}"
        exit 1
    fi
}

# Validate target contract if not "all"
if [ "$TARGET" != "all" ]; then
    valid=false
    for contract in "${CONTRACTS[@]}"; do
        if [ "$TARGET" == "$contract" ]; then
            valid=true
            break
        fi
    done
    
    if [ "$valid" == "false" ]; then
        echo "Error: Unknown contract '${TARGET}'"
        echo "Available contracts: ${CONTRACTS[*]}"
        exit 1
    fi
fi

# Build contracts
if [ "$TARGET" == "all" ]; then
    echo "Building all contracts..."
    cargo build --target wasm32-unknown-unknown --release
    echo "All contracts built successfully."
    
    # Optimize all WASM artifacts
    echo ""
    echo "Optimizing WASM artifacts for deployment..."
    for contract in "${CONTRACTS[@]}"; do
        if ! optimize_contract "$contract"; then
            echo "Error: Optimization failed for ${contract}"
            exit 1
        fi
    done
else
    build_contract "$TARGET"
fi

# Output summary with optimized artifact paths
echo ""
echo "================================================================"
echo "Build and optimization complete!"
echo ""
echo "Optimized WASM artifacts ready for deployment:"
for contract in "${CONTRACTS[@]}"; do
    wasm_path="${WASM_OUTPUT_DIR}/${contract}.wasm"
    if [ -f "$wasm_path" ]; then
        size=$(stat -c%s "$wasm_path" 2>/dev/null || stat -f%z "$wasm_path" 2>/dev/null || echo "unknown")
        echo "  - ${wasm_path} ($(numfmt --to=iec "$size" 2>/dev/null || echo "$size bytes"))"
    fi
done
echo ""
echo "Deploy with: ./scripts/deploy_testnet.sh"
echo "================================================================"
