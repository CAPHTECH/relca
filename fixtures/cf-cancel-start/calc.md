# Independent hand calculation — ADR 0041

A21 start-receipt supplement. Select prefix long start1 before slow start1; slow start writes marker=taken and notifies long on SECOND checked variable. Immediate cancel ACK2 cannot roll back that write/notification. Holder still free but long conflicts6. Removing only slow start makes long committed6. Marker LWW makes static start writer legal without allowing checked start writes.

Stage C: authored oracle, not observed implementation behavior.
