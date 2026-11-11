# Revisão de segurança do protótipo

Revisão feita com código e cofre descartável. O release público ainda não inclui este conjunto. Em um perfil temporário do Brave Origin, a extensão real enviou mensagens ao host real para consultar estado, parear, listar e ler uma entrada e recebeu recusa após bloquear. Nenhum perfil pessoal ou credencial real foi usado.

## Controles verificados

- O host exige pareamento por instalação, vincula o token ao cofre e à origem da extensão e aceita apenas URLs HTTPS que correspondam ao campo `url` da entrada.
- Revogação e bloqueio persistente impedem novos pedidos; o host verifica novamente a autorização após ler o segredo. Os testes de integração cobrem duas instalações isoladas, domínio semelhante, revogação e bloqueio durante a leitura.
- A extensão obtém a origem da aba pela API do navegador, confere a origem novamente antes de preencher e só injeta depois do clique do usuário. Senhas não são salvas em `browser.storage.local`; ali fica somente o token de pareamento.
- O Git recusa árvore suja e divergência antes de sincronizar. O teste com dois clones confirma que uma divergência não substitui o HEAD local.
- A interface não recebe o token do GitHub; a autenticação é delegada ao `gh`. A chave GPG e sua frase secreta permanecem sob controle do GPG e `pinentry`.

## Limites para validação antes de release

- Ainda é preciso testar preenchimento completo e falhas de formulário em perfis temporários reais de Brave e Chrome, com host registrado pelos manifestos do pacote. O Chrome estável instalado aqui não carregou a extensão por `--load-extension`, tanto em modo headless quanto em janela normal; por isso sua integração não foi validada nesse método.
- O bloqueio do host não limpa o cache de `gpg-agent` nem o acesso pelo terminal. Um pedido que já passou pela última verificação pode responder antes de o bloqueio ser aplicado.
- Atualizar ou remover uma entrada verifica o hash do arquivo cifrado, mas há uma janela entre a verificação e o subprocesso `pass`.
- O host é um processo do próprio usuário. Outro processo desse usuário pode invocá-lo; o token e a aprovação limitam acesso, mas não protegem contra um processo local que já controle o usuário ou seu chaveiro.
- O pacote tem SHA-256 no mesmo release, ainda sem assinatura independente. A extensão é distribuída como arquivo para carregamento manual; IDs e publicação em lojas não estão definidos.
- Nomes de entradas e histórico de commits do Git ficam visíveis para quem acessa o repositório privado. Nunca inclua backup da chave privada nem arquivos em texto claro nesse repositório.

## Critério para publicar

Concluir a matriz Brave/Chrome com cofre de teste, verificar instalação e atualização sem checkout local, e revisar os limites acima. Registrar o resultado observado antes de disparar o workflow de release.
