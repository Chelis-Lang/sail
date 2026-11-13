# sail-kani

Development groundwork for the experimental Kani backend selected by ADR 0109.
This first library slice decodes bounded observations from pinned Kani 0.68.0
and CBMC 6.11.0. It neither invokes Kani nor creates proof receipts, currentness,
cache authority or an ordinary verified result. Bounded native Linux decoder controls and independent review pass. CLI execution
and complete direct-Kani pilot qualification are separate gates. See ADR 0128.
