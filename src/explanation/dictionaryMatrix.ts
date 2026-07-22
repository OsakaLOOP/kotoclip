import type { DictionaryFormGroup, DictionaryLookup } from "../types";

export function formSupportsDictionary(
  form: DictionaryFormGroup | null | undefined,
  dictionaryName: string | null | undefined,
) {
  if (!form || !dictionaryName) return false;
  return form.dictionaries.some((item) => (
    item.dictionary_name === dictionaryName && item.available
  ));
}

export function dictionaryForForm(
  lookup: DictionaryLookup | null | undefined,
  formId: string | null | undefined,
  preferredDictionary?: string | null,
) {
  const form = lookup?.forms.find((item) => item.form_id === formId);
  if (!lookup || !form) return null;
  if (formSupportsDictionary(form, preferredDictionary)) return preferredDictionary ?? null;
  return lookup.dictionary_names.find((name) => formSupportsDictionary(form, name)) ?? null;
}

export function formForDictionary(
  lookup: DictionaryLookup | null | undefined,
  dictionaryName: string | null | undefined,
  preferredFormId?: string | null,
) {
  if (!lookup || !dictionaryName) return null;
  const preferred = lookup.forms.find((form) => form.form_id === preferredFormId);
  if (formSupportsDictionary(preferred, dictionaryName)) return preferred?.form_id ?? null;
  return lookup.forms.find((form) => formSupportsDictionary(form, dictionaryName))?.form_id ?? null;
}

export function dictionaryHasAnyForm(
  lookup: DictionaryLookup | null | undefined,
  dictionaryName: string,
) {
  return Boolean(formForDictionary(lookup, dictionaryName));
}
