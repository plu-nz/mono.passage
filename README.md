# Interface para GNU pass

> Nome provisório. Este repositório registra os requisitos e decisões do projeto antes da implementação.

## Objetivo

Criar uma experiência de cofre de credenciais que use o formato do GNU `pass`, preserve o acesso pelo terminal e funcione bem no Linux, nos navegadores Brave e Chrome e no Android. O usuário controla o armazenamento: o cofre é sincronizado por um repositório Git privado, hospedado em um serviço de sua escolha ou em infraestrutura própria.

## Requisitos confirmados

### Cofre e armazenamento

- Usar o GNU `pass` e seu formato de cofre: uma árvore de arquivos `.gpg` cifrados com OpenPGP.
- Manter compatibilidade com os comandos do `pass` no terminal, independentemente da interface gráfica.
- Sincronizar o cofre por Git privado, inclusive com a possibilidade de hospedar o remoto em um NAS ou servidor próprio.
- Manter a chave PGP privada fora do repositório Git. Cada dispositivo deve receber sua chave por um processo de configuração próprio.
- Detectar conflitos de sincronização e exigir resolução explícita quando duas alterações atingirem a mesma entrada.

### Linux e navegadores

- Oferecer uma interface desktop para consultar e gerenciar o cofre.
- Permitir preenchimento de credenciais no Brave e no Chrome por meio de extensões que acessem o mesmo cofre local.
- Evitar uma sessão independente do cofre em cada navegador. O aplicativo ou serviço local deve controlar o acesso, com pareamento e possibilidade de revogar cada extensão.
- Investigar desbloqueio do cofre pela digital, inclusive no primeiro acesso após iniciar o sistema. A verificação biométrica precisa liberar uma chave protegida; uma checagem isolada de digital não é suficiente.

### Android

- Permitir consultar e gerenciar o mesmo cofre `pass` no Android.
- Integrar com o preenchimento de senhas do sistema e avaliar o Credential Manager para passkeys em versões compatíveis do Android.
- Sincronizar com o mesmo repositório Git privado e usar autenticação local do dispositivo para proteger o acesso à chave.

### Tipos de credencial

- Suportar logins com usuário e senha.
- Suportar segredos TOTP para gerar códigos de autenticação de dois fatores.
- Avaliar compatibilidade de passkeys com o formato documentado pelo fork `agrahn/Android-Password-Store`, incluindo uso no desktop e sincronização entre dispositivos.

## Direção inicial

1. Validar um cofre de teste com `pass`, Git privado, Browserpass no Brave e Chrome, e o fork Android Password Store no celular.
2. Identificar as lacunas reais dessa combinação, especialmente experiência desktop, pareamento dos navegadores, biometria e resolução de conflitos.
3. Construir a interface e os componentes necessários mantendo os arquivos utilizáveis pelo `pass`.
4. Escolher a tecnologia da interface após validar os limites de integração. TypeScript com Tauri 2 é uma possibilidade por permitir compartilhar código com extensões; Flutter continua como alternativa.

## Decisões em aberto

- Nome definitivo e titularidade do repositório (`@xonha` ou `plunz`).
- Escopo do primeiro lançamento: interface Linux, extensão própria ou adaptação do Browserpass, e app Android próprio ou integração com o projeto existente.
- Modelo de desbloqueio biométrico no Linux, proteção da chave local e recuperação quando o leitor não estiver disponível.
- Convenção de arquivos para TOTP, metadados de sites e passkeys que preserve a compatibilidade com o `pass`.
- Política de sincronização: quando fazer pull/push, como apresentar conflitos e como recuperar alterações.
- Necessidade de uma interface web. O caso de uso principal é atendido por desktop, extensões e Android.

## Referências

- [GNU pass](https://www.passwordstore.org/)
- [Browserpass](https://github.com/browserpass/browserpass-extension)
- [Android Password Store (fork com passkeys)](https://github.com/agrahn/Android-Password-Store)
- [Formato de passkeys do Android Password Store](https://github.com/agrahn/Android-Password-Store/blob/develop/PasskeyStorage.md)
