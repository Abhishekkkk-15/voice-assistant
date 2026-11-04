use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use crate::{model::tools::Tool, utlis::config::AppConfig};

#[derive(Deserialize)]
pub struct WeatherArgs {
    city_name: String,
    country_code: Option<String>,
}

pub struct WeatherTool;

#[async_trait]
impl Tool for WeatherTool {
    fn name(&self) -> &str {
        "weather_tool"
    }

    fn description(&self) -> &str {
        "weather_tool is used to get the current weather of a city using city name and country name or code"
    }

    fn parameter_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "city_name": {
                    "type": "string",
                    "description": "The name of the city, e.g. Tokyo, Delhi, London"
                },
                "country_code": {
                    "type": "string",
                    "description": "The country code or name, e.g. IN, US, JP, UK"
                }
            },
            "required": ["city_name", "country_code"],
            "additionalProperties": false
        })
    }

    async fn run(&self, args: &str) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let config = AppConfig::global();
        let api_key = &config.weather_api;

        let arguments: WeatherArgs = serde_json::from_str(args)?;
        let city_name = arguments.city_name.trim();
        let country_code = arguments.country_code.as_deref().unwrap_or("").trim();

        // 1. Fetch current weather from OpenWeatherMap 2.5 API
        let weather_url = if country_code.is_empty() {
            format!("https://api.openweathermap.org/data/2.5/weather?q={city_name}&appid={api_key}&units=metric")
        } else {
            format!("https://api.openweathermap.org/data/2.5/weather?q={city_name},{country_code}&appid={api_key}&units=metric")
        };

        let mut res = reqwest::get(&weather_url).await?;

        // Fallback: if not found with country code, try with city name only
        if !res.status().is_success() && !country_code.is_empty() {
            let fallback_url = format!("https://api.openweathermap.org/data/2.5/weather?q={city_name}&appid={api_key}&units=metric");
            if let Ok(fallback_res) = reqwest::get(&fallback_url).await {
                if fallback_res.status().is_success() {
                    res = fallback_res;
                }
            }
        }

        let weather_data = res.json::<Value>().await?;

        // Check if API returned an error message
        if let Some(cod) = weather_data.get("cod") {
            let is_err = match cod {
                Value::Number(n) => n.as_i64().unwrap_or(200) != 200,
                Value::String(s) => s != "200",
                _ => false,
            };
            if is_err {
                let msg = weather_data["message"].as_str().unwrap_or("Failed to fetch weather");
                return Err(format!("OpenWeatherMap error: {msg}").into());
            }
        }

        let temp = weather_data["main"]["temp"].as_f64().unwrap_or(0.0);
        let feels_like = weather_data["main"]["feels_like"].as_f64().unwrap_or(temp);
        let humidity = weather_data["main"]["humidity"].as_i64().unwrap_or(0);
        let condition = weather_data["weather"][0]["description"].as_str().unwrap_or("clear");
        let resolved_city = weather_data["name"].as_str().unwrap_or(city_name);

        Ok(json!({
            "city": resolved_city,
            "temperature": format!("{temp:.1}°C"),
            "feels_like": format!("{feels_like:.1}°C"),
            "humidity": format!("{humidity}%"),
            "condition": condition
        }))
    }
}
