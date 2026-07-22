import assert from "node:assert/strict";
import test from "node:test";
import {
  dictionaryForForm,
  dictionaryHasAnyForm,
  formForDictionary,
  formSupportsDictionary,
} from "./dictionaryMatrix.ts";

const lookup = {
  dictionary_names: ["Crown", "大辞林", "小学馆", "Starter"],
  forms: [
    {
      form_id: "form:あの",
      dictionaries: [
        { dictionary_name: "Crown", available: true },
        { dictionary_name: "大辞林", available: false },
        { dictionary_name: "小学馆", available: true },
        { dictionary_name: "Starter", available: false },
      ],
    },
    {
      form_id: "form:彼の",
      dictionaries: [
        { dictionary_name: "Crown", available: false },
        { dictionary_name: "大辞林", available: true },
        { dictionary_name: "小学馆", available: true },
        { dictionary_name: "Starter", available: false },
      ],
    },
  ],
};

test("选择表记时保留可用词典，否则按词典优先级回退", () => {
  assert.equal(dictionaryForForm(lookup, "form:あの", "小学馆"), "小学馆");
  assert.equal(dictionaryForForm(lookup, "form:あの", "大辞林"), "Crown");
  assert.equal(dictionaryForForm(lookup, "form:彼の", "Crown"), "大辞林");
});

test("选择词典时保留可用表记，否则按表记优先级回退", () => {
  assert.equal(formForDictionary(lookup, "小学馆", "form:彼の"), "form:彼の");
  assert.equal(formForDictionary(lookup, "大辞林", "form:あの"), "form:彼の");
  assert.equal(formForDictionary(lookup, "Crown", "form:彼の"), "form:あの");
});

test("矩阵区分可联动暗显项与整个查询不可用的词典", () => {
  assert.equal(formSupportsDictionary(lookup.forms[0], "大辞林"), false);
  assert.equal(dictionaryHasAnyForm(lookup, "大辞林"), true);
  assert.equal(dictionaryHasAnyForm(lookup, "Starter"), false);
  assert.equal(formForDictionary(lookup, "Starter", "form:あの"), null);
});
