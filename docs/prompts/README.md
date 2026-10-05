# ADVERTENCIA: ARCHIVO - NO EJECUTAR ESTOS PROMPTS

**Estado: OBSOLETO (2026-10-05, auditoria exhaustiva del repo).**

Este directorio contiene prompts de agente escritos contra un estado del repo
que YA NO EXISTE. Ningun agente (ni humano) debe seguir estas instrucciones
como si fueran validas. Razones verificadas:

1. PROMPT_01 y PROMPT_03 ordenan corromper un invariante verificado:
   mandan cambiar WEIGHT_COUNT de 2863 a 2897 y los operadores de 13 a 14.
   El codigo real fija WEIGHT_COUNT == 2863 y OPS == 13 con tests
   (repair_nn_core/src/lib.rs), y model/current.txt (artefacto de produccion)
   tiene 2863 pesos. Aplicar el prompt rompe el build y hace que el Worker
   caiga a blocked_no_model.
2. Requisitos ya derrocados por decision del dueno: p. ej. "No eliminar
   legacy/" (eliminado, PART3 22) y la aritmetica de pesos de PROMPT_03 no
   cuadra por si misma.
3. Requisitos de infraestructura inexistente: CODEOWNERS, dependabot.yml y
   MongoDB no existen en el repo.
4. Numeros de items de DISCREPANCIES ya usados por items reales.

Los .md se conservan como registro historico de los prompts originales.
Los .pdf (duplicados renderizados de los .md) fueron eliminados por
divergencia potencial.

Si se quiere una version vigente de estos prompts, debe escribirse contra el
HEAD actual y validarse contra los tests del repo, no contra este archivo.
