# P22: aset-conditional-agency.broken-nondet

Hand oracle fixed before implementation (single edit). Unit: min.

Atomic sees gate true at3, calls Next [3,5): E5/W(3+1+2)=6. Static F guards Unknown; L(post)=1..1, L(Next) not_derived/state_guards. Independent nondet results admit rejected3/W4 and committed5/W6; F derived and L(Next)=0..1. Do not degrade O/P with Unknown.

Single mechanism edit: 'writes [keys, registered]\n      duration fixed 3min\n      atomic {\n        if gate==true { keys += key; registered = true; result committed }\n        else { result rejected_duplicate }\n      }' -> 'duration fixed 3min;result nondet{committed,rejected_duplicate}'.

Static failures have no analyzed time or invented zero counts. Computational cuts are not business rejection. Atomic refusal is a normal finish, not a requires refusal. A RED checkpoint is not semantic verification.
