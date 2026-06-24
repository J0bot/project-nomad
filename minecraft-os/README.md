# minecraft-os

Mod Fabric (Minecraft 1.21.11, client + serveur) qui fait de Minecraft une
SURFACE/DIMENSION du xerboxion-os. Le mod POSTe les events du jeu sur le bus du
xion (POST /emit), affiche un HUD des tsoins cote client, hooke Simple Voice
Chat. Grammaire : bion -> cubion -> ploxion -> xion.

Etat : SQUELETTE v0 -- buildable, demarre, features = stubs marques TODO.

## Versions figees (MC 1.21.11)

Dans gradle.properties. Re-confirmer avant build (cibles mouvantes) :

- minecraft 1.21.11
- fabric-loader 0.19.3 (meta.fabricmc.net/v2/versions/loader/1.21.11)
- yarn 1.21.11+build.6 (meta.fabricmc.net/v2/versions/yarn/1.21.11)
- fabric-api 0.141.4+1.21.11 (modrinth.com/mod/fabric-api/versions?g=1.21.11)
- fabric-loom 1.14-SNAPSHOT (github.com/FabricMC/fabric-loom/releases)
- gradle 8.14, java 21
- voicechat-api 2.1.12 compileOnly (maven.maxhenkel.de)

NOTE migration : Fabric arrete Yarn/Intermediary APRES 1.21.11. build.6 = la
derniere ligne stable. Version future = loom.officialMojangMappings() + remap.
Acces MC isole dans helpers dimPath/blockPath pour limiter le cout.

## Build

    gradle wrapper --gradle-version 8.14   # une fois (gradle local requis)
    ./gradlew build                        # -> build/libs/minecraft-os-<v>.jar

Java 21 obligatoire. NE PAS builder sur le boxion (VPS leger) -- build sur ton PC.

## Topics emis (payload = JSON metier double-encode en string)

- minecraft.command {player,dim,cmd,t} -> mc-adapter grave mc:<dim>:cmd:<seq> + mc.event
- block.break {block,dim,x,y,z} -> block-bion (sdk/src/block.rs)
- minecraft.block {op,dim,x,y,z,block,player,t} -> Nexus/carte
- minecraft.player {player,dim,kind,t}
- minecraft.chat {player,dim,msg,t}
- minecraft.world {seed,name,t} (le generateur, 1x au boot)
- voice.frame {player,dim,seq,t,bytes_len} (metadata only)

Double-encodage : body /emit = {topic, payload} ou payload = JSON metier en
STRING (host injecte verbatim). Cf. crates/xerboxion-host/src/serve.rs EmitBody.

## Bus URL

XionBridge POST vers ${XION_BUS_URL}/emit, defaut http://10.0.0.1:8730 (IP
INTERNE). Override env XION_BUS_URL (ou XION_URL). Serveur ailleurs (Jose joue
local, 25565 bloque) -> pointer xion.j0bot.ch (auth SSO, a trancher).

## Simple Voice Chat

compileOnly : le mod boote SANS SVC. Le runtime serveur doit fournir le mod SVC.
Enregistrement via META-INF/services/de.maxhenkel.voicechat.api.VoicechatPlugin.
V0 = metadata seule (pas l'audio Opus sur le bus, cap 64 KiB).

## Garde-fous (MEMORY : ne pas surcharger le VPS)

emits async (file bornee, jamais de POST dans le tick), drop-oldest si pleine,
residu SEULEMENT (modifs joueur), voice = metadata bornee.
