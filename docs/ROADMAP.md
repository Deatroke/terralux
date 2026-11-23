# Roadmap

Plataformas-alvo: Linux e Windows 11. Stack: Rust, `glam`, `rayon`, `serde`/RON; GUI com `winit` + `wgpu` + `egui`.

## Regra clean-room (vale para todos os marcos)
- Escrever o código a partir dos manuais (como especificação funcional) e do **comportamento observado** do Bryce.
- **Não** decompilar `Bryce 3D.exe` (nem com REA/Ghidra) para portar lógica.
- **Não** copiar nada do CD: texturas, presets, bibliotecas de céu/terreno/materiais, arte da UI, texto do manual.
- O Bryce é propriedade da DAZ 3D; a idade do programa não muda isso. O nome "Bryce" não é usado: **Terralux** é um nome provisório.
- Importar `.br3` (M4) é interoperabilidade para abrir **cenas do próprio usuário**; é opcional e isolada do resto.

## M1 — Renderizador por linha de comando ✅ (feito)
Workspace `core` / `render` / `cli`; cena em RON; céu, nuvens, névoa, água, terreno procedural e poliedro; PNG em qualquer resolução.
Falta de M1: poliedros extras e primitivas básicas (cubo/cilindro/cone/toroide), plano de terra, luzes pontuais.

## M2 — Terreno e materiais
Importar mapa de altura (PNG), pincel, filtros (erosão, terraço); sistema de materiais por canais; textura 3D procedural em árvore (base do Deep Texture Editor); booleanos e metaballs; BVH para malhas.

## M3 — GUI com preview em tempo real 🟡 (primeira versão pronta: `terralux-studio`)
Janela com `winit`+`wgpu`+`egui`; viewport (wireframe/sólido), paletas ao estilo do original, desfazer/refazer, preview progressivo do ray tracer (CPU ou compute shader); edição de céu/água/terreno ao vivo.

## M3.5 — Terrenos e cenas para jogo ✅ (v1.1)
Tela inicial, atalhos estilo Blender, prévia leve, 17 modelos de cena (6 relevos, 7 biomas), importar/exportar glTF/OBJ/STL/PLY, mapa de altura de 16 bits e CLI para agentes (`docs/CLI.md`).

## M4 — Importar `.br3` (opcional)
Só leitura de cenas do próprio usuário, a partir de arquivos de exemplo; formato documentado por observação. Fora do caminho crítico.

## M5 — Build para Windows 11 + distribuição 🟡 (CI de release Linux + Windows pronta, v1.0.0)
CI com Linux e Windows (MSVC), empacotamento (AppImage/zip/instalador), testes de render de referência.

## Depois
Path tracing com GI, render em GPU, denoiser, animação por keyframes, DOF, edição de vértices, escultura de terreno com pincel.
