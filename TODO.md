## TODO


### Integration user stories 

1. make the bot be able to tell you about the weather (using teh weather skill and exec tool ) 

tell me about the weather currently in austin texas  DONE 


## Add
add the cron skill and memory skill   





## function calling 


You are an agent named StarkBot who is able to respond and operate tools.   You will often be given a list of tools that you can call.  Always respond in json in the following format: 

{  body: string , tool_call: option< {  tool_name: String, tool_params: Object  } >   }



lets give the tools like using the openai tools schema spec and we should dynamically build it using the actual enabled tools .  one tool will be the 'skills ' tool and it will have nested within it al of the available skills 

