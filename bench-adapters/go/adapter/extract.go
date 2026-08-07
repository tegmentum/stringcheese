package adapter

import (
	"errors"
	"fmt"
	"os"
	"os/exec"
	"strings"
)

// extractCoreModule invokes `wasm-tools component unbundle` on the
// component `.wasm` at `componentPath`, writing the extracted core
// module(s) into `outDir`. The command creates `outDir` if it does not
// already exist.
//
// wasm-tools writes one file per embedded core module — for the
// StringCheese component, module 0 is the actual algorithm kernel
// (imports wasi_snapshot_preview1, exports the WIT interfaces) and
// module 1 is the small adapter shim that translates preview1 to
// preview2. We only need module 0 for execution; wazero's built-in
// wasi_snapshot_preview1 shim satisfies its imports directly.
//
// wasm-tools is *not* a Go dependency of this adapter — it is a
// separate Rust tool the repo already requires for the component
// build. If it is absent from PATH we return a diagnostic pointing at
// the install command from the top-level README.
func extractCoreModule(componentPath, outDir string) error {
	if _, err := exec.LookPath("wasm-tools"); err != nil {
		return errors.New(
			"wasm-tools not found on PATH — install with " +
				"`cargo install wasm-tools`, then rerun; needed once " +
				"to extract the core wasm module from the built component",
		)
	}
	if err := os.MkdirAll(outDir, 0o755); err != nil {
		return fmt.Errorf("mkdir %q: %w", outDir, err)
	}
	// `--module-dir` sets the output directory; a `--threshold` of 0
	// forces every embedded module to be extracted regardless of size.
	// We don't pipe stdout because unbundle's stdout is the resulting
	// "outer" component (with imports substituted for the modules) —
	// we don't need that here, so redirect to a temp file that gets
	// removed on success.
	tmpOut, err := os.CreateTemp("", "stringcheese-unbundle-*.wasm")
	if err != nil {
		return fmt.Errorf("create temp for unbundle: %w", err)
	}
	tmpPath := tmpOut.Name()
	_ = tmpOut.Close()
	defer os.Remove(tmpPath)

	cmd := exec.Command(
		"wasm-tools", "component", "unbundle",
		"--module-dir", outDir,
		"--threshold", "0",
		"-o", tmpPath,
		componentPath,
	)
	var stderr strings.Builder
	cmd.Stderr = &stderr
	if err := cmd.Run(); err != nil {
		return fmt.Errorf(
			"wasm-tools component unbundle failed: %w\nstderr: %s",
			err, stderr.String(),
		)
	}
	return nil
}
