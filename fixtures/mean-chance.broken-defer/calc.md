# opaque defer だけを足す

Foo は Actor に当たらず process 全体が影響不明になる。W-CUT-2。
ADR 0078 の5.2に従い全meanの値・下界を撤回し not_configured。
他の品質の prior reason 保持とは区別する。reachable done は satisfied/exit 0。
