# klean

CLI segura e eficiente para limpar ambientes de desenvolvimento.

## Instalação

**Homebrew (macOS/Linux)**
```sh
brew tap klean-cli/klean
brew install klean
```

**Cargo**
```sh
cargo install klean
```

**Scoop (Windows)**
```sh
scoop bucket add klean https://github.com/klean-cli/scoop-bucket
scoop install klean
```

**Download direto** — baixe o binário mais recente em [Releases](https://github.com/danil0ws/klean/releases).

## Uso

```
klean                   # menu interativo (TUI)
klean .                 # escanear diretório atual
klean ~/projects        # escanear um caminho específico
klean globals           # gerenciar pacotes instalados globalmente
klean update            # auto-atualizar para a versão mais recente
klean undo              # restaurar a última sessão deletada da lixeira
klean trash             # listar sessões na lixeira
klean trash --empty     # esvaziar lixeira permanentemente
klean stats             # exibir histórico de limpezas
```

### Menu interativo

Executar `klean` sem argumentos abre o menu TUI:

| # | Ação |
|---|------|
| 1 | Limpar artefatos — remove caches de build, node_modules, dist, … |
| 2 | Pacotes globais — gerenciar npm/cargo/pip/brew/binários |
| 3 | Estatísticas — histórico de limpezas |
| 4 | Atualizar *(exibido apenas quando há uma versão nova disponível)* |
| 4/5 | Sair |

Navegue com `↑↓` ou `jk`, confirme com `Enter` ou pressione o número da opção.

### Pacotes globais

Pressione `u` para verificar pacotes desatualizados (badges aparecem na coluna Versão).  
Pressione `Espaço` para selecionar, `a` para selecionar tudo, `Enter` para remover selecionados.  
Pressione `s` para alternar ordenação entre gerenciador e tamanho.

## O que é limpo

klean remove: `node_modules`, `target`, `.cache`, `dist`, `build`, `.next`,
`.nuxt`, `__pycache__`, `.venv`, `vendor` e muito mais — para mais de 20 linguagens
e frameworks. Veja `klean.toml` para configuração por projeto.

## Configuração (`klean.toml`)

Coloque um `klean.toml` na raiz de um projeto para personalizar o comportamento:

```toml
[project]
backup = true          # mover para lixeira em vez de deletar
backup_dir = ".trash"  # localização customizada da lixeira

[[ignore]]
path = "dist"          # nunca tocar neste diretório
```

## Auto-atualização

```sh
klean update           # atualizar para a versão mais recente
klean update --dry-run # apenas visualizar
```

klean verifica a versão mais recente no crates.io e instala via
`cargo install` se o cargo estiver disponível, ou baixa o binário pré-compilado
do GitHub Releases caso contrário.

## Licença

MIT OR Apache-2.0
