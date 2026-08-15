# ECLSS Reliability Profiles

`eclss-components.v1.json` is the first runtime reliability profile for Underhill's continuous ECLSS. It records four representative ORUs whose MTBF, MTTR, and operational K-factor values are published in ICES-2025-127 Tables 2, 8, 9, and 10.

The profile deliberately keeps three evidence classes separate:

- `published_mads_derived_aggregate`: values printed in the public conference paper;
- `underhill_*_assumption`: modeled duty cycle, installed quantity, and initial Mars-base spare inventory;
- runtime state: seeded failure draws, operating exposure, component condition, work orders, and consumed spares.

The source paper identifies ISS MADS as the primary repository and says the reliability database is incomplete. This profile is therefore not raw MADS, a complete ECLSS reliability block diagram, or a prediction for Mars-qualified hardware. It is a traceable initial distribution for validation and sensitivity analysis.

The runtime uses the paper's constant-failure-rate/exponential formulation. Random-generator state and each sampled component clock are private checkpoint state; agents receive configured rates and observed condition but not the exact future failure draw. Reference MTTR drives remove-and-replace work duration. The paper warns that those durations do not include all troubleshooting, gathering, stowing, access, scheduling, or crew-availability delay; those logistics layers remain future work. Finite spare inventories can be replenished through a bounded external-logistics action so the plant has no terminal mission state; future ISRU/logistics PEAs will own that boundary physically.
