#!/usr/bin/env python3
# Régénère/étend collections/civilization-papers.json depuis MDPI (open-access, Crossref).
# Augmente ROWS ou ajoute des domaines pour enrichir. python3 build-civilization.py
import urllib.request,urllib.parse,json,time
UA={"User-Agent":"xerboxion-nomad/1.0 (mailto:j0bot@pm.me)"}; ROWS=10
DOMAINS=[("water","Eau & assainissement","💧","water purification sanitation filtration review"),
 ("food","Nourriture & agriculture","🌾","sustainable agriculture crop yield food preservation review"),
 ("energy","Énergie & électricité","⚡","renewable energy generator electricity storage review"),
 ("medicine","Médecine & santé","⚕️","essential medicine antibiotics public health review"),
 ("materials","Matériaux & métallurgie","⛓️","metallurgy steel alloy materials processing review"),
 ("chemistry","Chimie & procédés","⚗️","industrial chemistry synthesis process engineering review"),
 ("construction","Construction & abri","🏗️","construction materials concrete structural engineering review"),
 ("manufacturing","Fabrication & machines","⚙️","manufacturing machining mechanical engineering review"),
 ("electronics","Électronique & calcul","💻","semiconductor electronics microcontroller computing review"),
 ("comms","Communications & réseaux","📡","wireless communication radio network protocol review"),
 ("transport","Transport","🚜","internal combustion engine vehicle transportation review"),
 ("textiles","Textiles & polymères","🧵","textile fiber polymer synthesis review"),
 ("preservation","Conservation & stockage","🥫","food preservation cold storage drying fermentation review"),
 ("agronomy","Sols & élevage","🐄","soil fertility livestock animal husbandry agronomy review"),
 ("knowledge","Savoir & mesure","📐","metrology measurement standards science education review")]
def q(query):
    u="https://api.crossref.org/works?"+urllib.parse.urlencode({"filter":"member:1968,type:journal-article","query":query,"rows":ROWS,"sort":"relevance","select":"title,author,container-title,published,DOI,URL,link","mailto":"j0bot@pm.me"})
    try: d=json.load(urllib.request.urlopen(urllib.request.Request(u,headers=UA),timeout=30))
    except Exception: return []
    out=[]
    for it in d["message"]["items"]:
        pdf=next((L["URL"] for L in it.get("link",[]) if "/pdf" in L.get("URL","")),"")
        dp=(it.get("published",{}) or {}).get("date-parts",[[None]])
        out.append({"doi":it.get("DOI",""),"title":(it.get("title") or ["?"])[0],
            "authors":[(a.get("given","")+" "+a.get("family","")).strip() for a in it.get("author",[])[:3]],
            "journal":(it.get("container-title") or [""])[0],"year":dp[0][0] if dp and dp[0] else None,
            "url":it.get("URL",""),"pdf":pdf})
    return out
cats=[{"slug":s,"name":n,"icon":i,"query":qq,"papers":q(qq)} for s,n,i,qq in DOMAINS for _ in [time.sleep(1.2)]]
tot=sum(len(c["papers"]) for c in cats)
json.dump({"spec_version":"2026-06-30","collection":"civilization-papers","title":"Reconstruire une civilisation","source":"MDPI (open-access CC-BY) via Crossref","counts":{"domains":len(cats),"papers":tot},"categories":cats},open("civilization-papers.json","w"),ensure_ascii=False,indent=1)
print(tot,"papiers /",len(cats),"domaines")
