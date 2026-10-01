use crate::domain::cross_file::context::TypeContext;
use crate::domain::frameworks::django::DjangoTypeProvider;
use crate::domain::frameworks::provider::{FrameworkTypeProvider, MethodType};
use crate::type_inference::Type;

impl FrameworkTypeProvider for DjangoTypeProvider {
    fn get_type(&self, symbol: &str, _context: &TypeContext) -> Option<Type> {
        // Check if symbol is a model
        if self.models.contains_key(symbol) {
            Some(Type::Instance {
                name: symbol.to_string(),
                module: Some("models".to_string()),
                type_args: Vec::new(),
            })
        } else {
            None
        }
    }

    fn get_attribute_type(&self, base_type: &Type, attr: &str) -> Option<Type> {
        if let Type::Instance { name, .. } = base_type {
            if let Some(model) = self.models.get(name) {
                if let Some(field) = model.fields.get(attr) {
                    return Some(self.field_type_to_type(&field.field_type));
                }
            }
        }
        None
    }

    fn get_method_signature(&self, base_type: &Type, method: &str) -> Option<MethodType> {
        if let Type::Instance { name, .. } = base_type {
            // Check if it's a model instance or QuerySet
            let is_queryset = name.ends_with("QuerySet");
            let model_name = if is_queryset {
                // Extract model name from "UserQuerySet" -> "User"
                name.strip_suffix("QuerySet").unwrap_or(name)
            } else {
                name.as_str()
            };

            // Check if this is a known model
            if self.models.contains_key(model_name) || is_queryset {
                return match method {
                    // QuerySet methods that return QuerySet
                    "filter" | "exclude" | "select_related" | "prefetch_related" | "annotate"
                    | "order_by" | "distinct" | "defer" | "only" | "using"
                    | "select_for_update" => Some(MethodType {
                        params: vec![], // **kwargs not modeled yet
                        return_type: Type::Instance {
                            name: format!("{}QuerySet", model_name),
                            module: None,
                            type_args: Vec::new(),
                        },
                        is_async: false,
                    }),

                    // get() returns model instance
                    "get" => Some(MethodType {
                        params: vec![],
                        return_type: Type::Instance {
                            name: model_name.to_string(),
                            module: None,
                            type_args: Vec::new(),
                        },
                        is_async: false,
                    }),

                    // all() returns QuerySet
                    "all" => Some(MethodType {
                        params: vec![],
                        return_type: Type::Instance {
                            name: format!("{}QuerySet", model_name),
                            module: None,
                            type_args: Vec::new(),
                        },
                        is_async: false,
                    }),

                    // first() / last() return Optional[Model]
                    "first" | "last" => Some(MethodType {
                        params: vec![],
                        return_type: Type::Optional(Box::new(Type::Instance {
                            name: model_name.to_string(),
                            module: None,
                            type_args: Vec::new(),
                        })),
                        is_async: false,
                    }),

                    // create() returns model instance
                    "create" => Some(MethodType {
                        params: vec![],
                        return_type: Type::Instance {
                            name: model_name.to_string(),
                            module: None,
                            type_args: Vec::new(),
                        },
                        is_async: false,
                    }),

                    // get_or_create() returns tuple (Model, bool)
                    "get_or_create" => Some(MethodType {
                        params: vec![],
                        return_type: Type::Tuple(vec![
                            Type::Instance {
                                name: model_name.to_string(),
                                module: None,
                                type_args: Vec::new(),
                            },
                            Type::Bool,
                        ]),
                        is_async: false,
                    }),

                    // update_or_create() returns tuple (Model, bool)
                    "update_or_create" => Some(MethodType {
                        params: vec![],
                        return_type: Type::Tuple(vec![
                            Type::Instance {
                                name: model_name.to_string(),
                                module: None,
                                type_args: Vec::new(),
                            },
                            Type::Bool,
                        ]),
                        is_async: false,
                    }),

                    // count() returns int
                    "count" => Some(MethodType {
                        params: vec![],
                        return_type: Type::Int,
                        is_async: false,
                    }),

                    // exists() returns bool
                    "exists" => Some(MethodType {
                        params: vec![],
                        return_type: Type::Bool,
                        is_async: false,
                    }),

                    // delete() returns tuple (int, dict)
                    "delete" => Some(MethodType {
                        params: vec![],
                        return_type: Type::Tuple(vec![
                            Type::Int,
                            Type::Dict(Box::new(Type::Str), Box::new(Type::Int)),
                        ]),
                        is_async: false,
                    }),

                    // update() returns int
                    "update" => Some(MethodType {
                        params: vec![],
                        return_type: Type::Int,
                        is_async: false,
                    }),

                    // values() / values_list() return QuerySet (simplified)
                    "values" | "values_list" => Some(MethodType {
                        params: vec![],
                        return_type: Type::Instance {
                            name: format!("{}QuerySet", model_name),
                            module: None,
                            type_args: Vec::new(),
                        },
                        is_async: false,
                    }),

                    // aggregate() returns dict
                    "aggregate" => Some(MethodType {
                        params: vec![],
                        return_type: Type::Dict(Box::new(Type::Str), Box::new(Type::Any)),
                        is_async: false,
                    }),

                    // Model instance methods
                    "save" if !is_queryset => Some(MethodType {
                        params: vec![],
                        return_type: Type::None,
                        is_async: false,
                    }),

                    "refresh_from_db" if !is_queryset => Some(MethodType {
                        params: vec![],
                        return_type: Type::None,
                        is_async: false,
                    }),

                    _ => None,
                };
            }
        }
        None
    }

    fn framework_name(&self) -> &str {
        "Django"
    }
}
