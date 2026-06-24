;; Test fixture: a ploxion that IMPORTS the gated host power `xerboxion.plc_fetch`
;; but DOES NOT declare the `net.fetch` capability in its manifest. The host MUST
;; reject it cleanly at load (least authority / consent) — it tried to grab a
;; power it never asked for. This is the negative half of the capability gate.
;;
;; It exports the full PLC v1 surface so it would load fine EXCEPT for the
;; undeclared import; that isolates the gate as the reason for rejection.
(module
  ;; The gated import it is NOT entitled to (no net.fetch in its manifest).
  (import "xerboxion" "plc_fetch"
    (func $plc_fetch (param i32 i32 i32 i32 i32 i32) (result i64)))

  (memory (export "memory") 1)

  ;; Manifest JSON at offset 16, length 55 — NOTE: no "capabilities" field.
  ;; {"id":"sneaky-fetcher","version":"1.0.0","provides":[]}
  (data (i32.const 16) "{\"id\":\"sneaky-fetcher\",\"version\":\"1.0.0\",\"provides\":[]}")

  ;; plc_manifest() -> packed(ptr=16, len=55) = (16<<32)|55 = 68719476791
  (func (export "plc_manifest") (result i64)
    (i64.const 68719476791))

  ;; alloc(len) -> ptr. Returns a fixed scratch pointer; never really used here.
  (func (export "alloc") (param i32) (result i32)
    (i32.const 1024))

  (func (export "plc_init"))
  (func (export "plc_health") (result i32) (i32.const 0))
  (func (export "plc_goodbye"))
)
