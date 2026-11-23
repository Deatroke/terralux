# Lista-alvo de funcionalidades

Fonte: **índice e conceitos** dos manuais do Bryce 3D de 1997, usados só como **especificação funcional**
("o que o programa faz"). Nada de código, arte, presets, texturas ou texto do CD é copiado.
Legenda: ✅ feito · 🟡 parcial · ⬜ pendente

## Ambiente
- ✅ Céu procedural com gradiente, sol (direção, cor, intensidade) e brilho
- ✅ Nuvens em plano infinito (ruído fBm, auto-sombreamento)
- ✅ Névoa/haze de distância
- 🟡 Sky & Fog Lab completo (modos de céu, bandas de nuvem, arco-íris, sombras de nuvem, névoa por altura)
- ⬜ Neblina/volumétrica (Volume Materials, "Visible Light" / raios de luz)

## Planos infinitos
- ✅ Plano de água (ondas procedurais, Fresnel, reflexo, refração e absorção por cor)
- ⬜ Plano de terra (Ground) com material
- ⬜ Plano de nuvem como objeto (Cloud plane)
- ⬜ Lajes infinitas (Infinite Slabs)

## Terreno (Terrain Editor)
- ✅ Heightfield procedural (fBm + ridged), ilha com máscara e costa deformada
- ✅ Ray marching seguro (limite de gradiente) com refino por bissecção
- ⬜ Importar mapa de altura em tons de cinza (PNG/16 bits)
- ⬜ Pintura de terreno (pincel), filtros (erosão, suavizar, terraços)
- ⬜ Recorte (clipping), cor por altura/declive editável
- ⬜ Salvar/carregar bibliotecas de terreno

## Objetos
- ✅ Esfera e poliedros convexos (tetra, octa, icosaedro)
- ⬜ Primitivas: cubo, cilindro, cone, toroide, disco, pirâmide
- ⬜ Pedras (Stones) procedurais, treliças simétricas (Symmetrical Lattices)
- ⬜ Metaballs e objetos booleanos (união, interseção, diferença)
- ⬜ Pict objects (imagem + canal alfa), caminhos geométricos, importação OBJ
- ⬜ Hierarquia: agrupar, ligar (link), randomizar

## Materiais e texturas (Materials Lab / Deep Texture Editor)
- 🟡 Material básico (cor, reflexo, especular)
- ⬜ Canais de superfície: difusão, ambiente, especular, halo, metalicidade, bump, transparência, refração, reflexo
- ⬜ Materiais de volume (cor, densidade, suavidade de borda, "fuzzy")
- ⬜ Editor de textura 3D procedural em árvore (componentes de cor/valor/textura, mapeamento, combinação)
- ⬜ Biblioteca de materiais própria (presets autorais)

## Luzes e câmera
- ✅ Luz solar direcional com sombras
- ⬜ Luzes pontuais/spot/área, gels (projeção de máscara), luz visível
- ✅ Câmera perspectiva (posição, alvo, FOV, exposição)
- ⬜ DOF, ortográfica, painéis de câmera múltiplos

## Renderização
- ✅ Render offline em CPU multi-thread, resolução livre, antialiasing por amostras
- ✅ Saída PNG com tonemapping ACES
- ⬜ Qualidade/antialiasing configurável por região, render de região
- ⬜ Path tracing (GI), iluminação baseada em imagem, denoiser
- ⬜ Render em GPU (wgpu) para preview e final
- ⬜ Saída 16 bits/EXR, canal alfa, camadas (Z, normal)

## Interface (Bryce clássico)
- ✅ Janela de trabalho com preview ray-traced progressivo, wireframe e modo combinado
- ✅ Interface própria, cybercore por padrão, com 5 temas prontos (ciano/magenta, verde, synthwave, âmbar, Frutiger Aero com arte do pacote AeroMedia, CC BY 4.0) e tema personalizado (cores, estilo, arredondamento, brilho, neon; exportar/importar): paleta por abas Criar / Editar / Céu, gaveta de biblioteca, coluna de visão e render, inspetor contextual
- ✅ Modo Simples e Avançado (lembrado), guia “Comece aqui”, setas de órbita e zoom sobre a cena, aviso com desfazer em vez de confirmação
- ✅ Vistas e pontos de memória de câmera; Sky & Fog; Terrain Editor (aba Elevação)
- ✅ Seleção por clique, mover/girar/escalar por arraste, desfazer/refazer, salvar/abrir cena, janela de render com salvar PNG
- ⬜ Terrain Editor: abas Filtragem e Imagem, pincel
- ⬜ Materials Lab (hoje: material simples no inspetor)
- ⬜ Nano-Preview em miniatura, Marker Pen (anotação), biblioteca de predefinições em disco

## Biblioteca
- ✅ Painel de assets com miniaturas (formas, divertidos, terrenos por mapa de altura, modelos OBJ/DXF, texturas, imagens recortadas)
- ✅ Malhas triangulares com BVH, texturas de imagem (planar/caixa/esférica), plano infinito, Picture Object com alfa
- ⬜ Gels (luz com projeção), editor de textura no inspetor para mais de um canal
- ❌ Fora do escopo: `.obp`, `.mat`, `.bsk`, `.br3`

## Animação
- ⬜ Keyframes, curvas de mapeamento de tempo, câmera e materiais animados, render de sequência

## Arquivos
- ✅ Cena nativa em RON (texto, versionável)
- ⬜ Importação opcional de `.br3` (somente para abrir cenas próprias; ver ROADMAP M4)
- ⬜ Exportar OBJ/DXF
