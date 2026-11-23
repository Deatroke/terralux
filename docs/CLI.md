# CLI do Terralux (para pessoas e agentes)

Binário: `terralux`. Tudo que a interface faz com cena, terreno e arquivos 3D também se faz por aqui, sem janela.

## Contrato (o que um agente pode assumir)

- **`--json`** (global) imprime **um** objeto JSON no stdout: `{"ok": true, ...}` em caso de sucesso, `{"ok": false, "error": "..."}` em caso de falha.
- Progresso e logs vão para o **stderr**; o stdout só tem o resultado.
- **Código de saída:** `0` sucesso · `1` a operação falhou · `2` argumentos inválidos (erro do `clap`).
- **Cenas:** `.ron` (padrão) ou `.json` (pela extensão). `-` lê a cena do stdin. Comandos que alteram a cena gravam nela mesma, a menos que você passe `-o`.
- **Unidades e eixos:** metros, **Y para cima**, destro. Olhando para +Z, a direita da imagem é −x.
- Alterações são **atômicas**: se `set` deixar a cena inválida (tipo errado, campo inexistente), nada é gravado.

## Receita rápida

```bash
terralux templates --json                      # ids dos modelos prontos
terralux new jogo-montanhas -o mapa.ron        # cria a cena
terralux set mapa.ron terrain.peak_height=90 terrain.seed=7
terralux add mapa.ron skull --name Totem --pos 0,40,0 --size 6 --color '#ffcc66'
terralux add mapa.ron ./rochas/pedra.glb --size 12       # modelo externo
terralux validate mapa.ron                     # sai com 1 se algo estiver quebrado
terralux export mapa.ron -o mapa.glb           # Unity / Unreal / Godot / Blender
terralux export-terrain mapa.ron -o altura.raw --size 1025   # mapa de altura 16 bits
terralux render mapa.ron --fast --width 960 --height 540 -o previa.png
```

## Comandos

| Comando | O que faz |
|---|---|
| `templates` | Lista os modelos de cena (cenários e terrenos para jogo). |
| `new <modelo> [-o f]` | Cria uma cena a partir de um modelo. Sem `-o`, imprime. |
| `from-heightmap <img> -o f [--half-size] [--height] [--template]` | Cena cujo terreno vem de um mapa de altura (PNG/TIFF 16 bits, RAW/R16 quadrado). |
| `info <cena>` | Terreno, objetos, câmera, sol, céu. Com `--json`, estruturado. |
| `validate <cena>` | Confere arquivos de modelo/imagem e valores; `1` se houver erros. |
| `get <cena> <caminho>` | Lê um valor. |
| `set <cena> caminho=valor… [-o f]` | Altera valores. |
| `add <cena> <forma\|arquivo> […]` | Adiciona objeto: `--name --pos --rot --size --color --reflect`. |
| `remove <cena> <índice\|nome>` | Remove um objeto. |
| `export <cena> -o f` | `.glb`, `.obj` (+ `.mtl`) ou `.stl`. Opções: `--terrain-res --no-terrain --no-objects --water --no-camera`. |
| `export-terrain <cena> -o f [--size n]` | Mapa de altura `.png` (16 bits) ou `.raw` (16 bits, little-endian). |
| `convert <in> -o out` | Converte modelo entre formatos, mantendo escala e posição. |
| `inspect <modelo>` | Triângulos, materiais e dimensões de um arquivo. |
| `render <cena> [--fast] …` | Gera PNG. `--fast` = prévia leve (sem sombras nem reflexos recursivos). |
| `example` | Cena de demonstração (RON, ou JSON com `--json`). |

## Caminhos de `get`/`set`

São os campos da cena em JSON, separados por ponto. Em listas, use o índice **ou o nome** do item.

```bash
terralux get cena.ron terrain.kind
terralux set cena.ron terrain.kind='"Canyon"'            # enums: texto JSON entre aspas
terralux set cena.ron objects.Totem.position=0,55,0      # x,y,z vira lista
terralux set cena.ron objects.0.material.color=[1,0.5,0]
terralux set cena.ron water=null                         # remove a água
terralux set cena.ron sun.elevation_deg=12 sky.stars=0.8
```

Um caminho errado lista os campos disponíveis na mensagem de erro. Para ver a estrutura inteira: `terralux example --json`.

## Formatos

| Direção | Formatos |
|---|---|
| Importar objetos | glTF/GLB, OBJ, STL, PLY, DXF; imagens PNG/JPG/WebP como recortes |
| Importar terreno | PNG/TIFF 8 ou 16 bits, RAW/R16 quadrado |
| Exportar cena | GLB (com câmera e sol), OBJ + MTL, STL |
| Exportar terreno | PNG 16 bits, RAW 16 bits |

**Limites honestos:** FBX e `.blend` são formatos fechados, sem leitor/escritor livre confiável. O caminho é **glTF/GLB**, que Blender, Unity, Unreal e Godot importam. "Editar" um arquivo 3D aqui significa importar, posicionar, trocar material e exportar; não há edição de vértices. Imagens recortadas (`Picture`) não são exportadas.

### Unity
- `export-terrain --size 1025` (ou 513, 2049, 4097) e importe como RAW: *Depth 16 bit, Byte Order Windows*. Use `extent_m` como largura/comprimento e `terrain_height_m` como altura (vêm no JSON).
- A Unity é canhota: o relevo chega espelhado em x. Para o GLB, a importação já corrige.

## Smoke test

`./scripts/cli-smoke.sh` exercita o fluxo completo (criar, editar, exportar, reimportar, erros). Use `TERRALUX=caminho/do/binario` para outro binário.
